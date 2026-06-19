//! Process plugin runtime foundation.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::notification::NotificationLevel;

use super::{
    PluginLoadError, PluginMessage, PluginProcess, PluginRegistry, PluginRequest, PluginResponse,
    read_frame, write_frame,
};

const INITIALIZE_REQUEST_ID: u64 = 1;
const FIRST_COMMAND_REQUEST_ID: u64 = 2;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const PROTOCOL_VERSION: u64 = 1;
const EDITOR_CAPABILITIES: &[&str] = &[
    "editor/notify",
    "editor/getActiveBuffer",
    "editor/getBufferText",
    "editor/getConfig",
    "editor/applyEdit",
];

/// Minimal runtime handle for a single process-backed plugin.
#[derive(Debug)]
pub struct PluginProcessRuntime {
    child: Child,
    stdin: ChildStdin,
    stdout: Option<BufReader<ChildStdout>>,
}

impl PluginProcessRuntime {
    /// Starts a plugin process from its manifest process config.
    pub fn spawn(process: &PluginProcess) -> Result<Self, PluginLoadError> {
        Self::spawn_in(process, Path::new("."))
    }

    /// Starts a plugin process from its manifest process config in `current_dir`.
    pub fn spawn_in(process: &PluginProcess, current_dir: &Path) -> Result<Self, PluginLoadError> {
        let mut command = Command::new(&process.command);
        command.current_dir(current_dir);
        command.args(&process.args);
        command.envs(&process.env);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::null());

        let mut child = command
            .spawn()
            .map_err(|error| PluginLoadError::runtime(error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginLoadError::runtime("plugin process missing stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginLoadError::runtime("plugin process missing stdout"))?;

        Ok(Self {
            child,
            stdin,
            stdout: Some(BufReader::new(stdout)),
        })
    }

    /// Sends a protocol message to the process.
    pub fn send(&mut self, message: &PluginMessage) -> Result<(), PluginLoadError> {
        write_frame(&mut self.stdin, message)?;
        self.stdin
            .flush()
            .map_err(|error| PluginLoadError::runtime(error.to_string()))
    }

    /// Receives a protocol message from the process.
    pub fn recv(&mut self) -> Result<PluginMessage, PluginLoadError> {
        read_frame(
            self.stdout
                .as_mut()
                .ok_or_else(|| PluginLoadError::runtime("plugin process reader already started"))?,
        )
    }

    fn take_stdout(&mut self) -> Result<BufReader<ChildStdout>, PluginLoadError> {
        self.stdout
            .take()
            .ok_or_else(|| PluginLoadError::runtime("plugin process reader already started"))
    }

    /// Stops the process.
    pub fn shutdown(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

/// Process lifecycle state for one plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginProcessState {
    /// The plugin has no process configuration.
    NotConfigured,
    /// The process is starting.
    Starting,
    /// The process initialized successfully.
    Running,
    /// The process failed to start or initialize.
    Failed(String),
    /// The process was stopped by the editor.
    Stopped,
}

/// Public status for a process-backed plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginProcessStatus {
    /// Plugin name.
    pub plugin: String,
    /// Current lifecycle state.
    pub state: PluginProcessState,
}

/// User-facing status entry for a loaded plugin process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginStatusEntry {
    /// Plugin name.
    pub plugin: String,
    /// Current lifecycle state.
    pub state: PluginProcessState,
    /// Advertised plugin capabilities.
    pub capabilities: Vec<String>,
    /// Last failure message, when the plugin is failed.
    pub error: Option<String>,
}

/// Event emitted by a process plugin reader.
#[derive(Clone, Debug, PartialEq)]
pub enum PluginRuntimeEvent {
    /// A request message was received from a plugin process.
    RequestReceived {
        /// Plugin name.
        plugin: String,
        /// Request payload.
        request: crate::plugin::PluginRequest,
    },
    /// A response message was received.
    ResponseReceived {
        /// Plugin name.
        plugin: String,
        /// Response payload.
        response: crate::plugin::PluginResponse,
    },
    /// A notification message was received.
    NotificationReceived {
        /// Plugin name.
        plugin: String,
        /// Notification payload.
        notification: crate::plugin::PluginNotification,
    },
    /// The plugin process exited or closed stdout.
    ProcessExited {
        /// Plugin name.
        plugin: String,
    },
    /// MessagePack or framing failed.
    ProtocolError {
        /// Plugin name.
        plugin: String,
        /// Error message.
        error: String,
    },
    /// Runtime I/O failed.
    RuntimeError {
        /// Plugin name.
        plugin: String,
        /// Error message.
        error: String,
    },
    /// A pending request exceeded its timeout.
    RequestTimedOut {
        /// Plugin name.
        plugin: String,
        /// Request id.
        id: u64,
        /// Request method.
        method: String,
    },
    /// A pending request failed because the process failed or exited.
    RequestFailed {
        /// Plugin name.
        plugin: String,
        /// Request id.
        id: u64,
        /// Request method.
        method: String,
        /// Failure message.
        error: String,
    },
}

/// Handles a plugin runtime event that targets editor-owned side effects.
pub fn handle_runtime_event(event: &PluginRuntimeEvent) -> bool {
    match event {
        PluginRuntimeEvent::NotificationReceived {
            plugin,
            notification,
        } if notification.method == "editor/notify" => {
            handle_notify_notification(plugin, notification)
        }
        PluginRuntimeEvent::NotificationReceived {
            plugin,
            notification,
        } => {
            tracing::debug!(
                plugin,
                method = notification.method,
                "ignoring unknown plugin notification"
            );
            false
        }
        PluginRuntimeEvent::RequestReceived { .. }
        | PluginRuntimeEvent::ResponseReceived { .. }
        | PluginRuntimeEvent::ProcessExited { .. }
        | PluginRuntimeEvent::ProtocolError { .. }
        | PluginRuntimeEvent::RuntimeError { .. }
        | PluginRuntimeEvent::RequestTimedOut { .. }
        | PluginRuntimeEvent::RequestFailed { .. } => false,
    }
}

fn handle_notify_notification(
    plugin: &str,
    notification: &crate::plugin::PluginNotification,
) -> bool {
    let Some(message) = notification
        .params
        .get("message")
        .and_then(|value| value.as_str())
    else {
        tracing::warn!(plugin, "plugin notification missing message");
        return false;
    };
    let level = match notification
        .params
        .get("level")
        .and_then(|value| value.as_str())
    {
        Some("info") | None => NotificationLevel::Info,
        Some("warn") | Some("warning") => NotificationLevel::Warn,
        Some("error") => NotificationLevel::Error,
        Some(other) => {
            tracing::warn!(
                plugin,
                level = other,
                "plugin notification used unknown level"
            );
            NotificationLevel::Warn
        }
    };

    crate::globals::enqueue_notification(level, format!("{plugin}: {message}"))
}

#[derive(Debug)]
struct ManagedPluginProcess {
    runtime: Option<PluginProcessRuntime>,
    reader: Option<JoinHandle<()>>,
    state: PluginProcessState,
    capabilities: Vec<String>,
}

#[derive(Clone, Debug)]
struct PendingPluginRequest {
    plugin: String,
    id: u64,
    method: String,
    started_at: Instant,
    timeout: Duration,
}

impl PendingPluginRequest {
    fn timed_out(&self, now: Instant) -> bool {
        now.duration_since(self.started_at) >= self.timeout
    }
}

/// Runtime manager for process-backed plugins.
#[derive(Debug)]
pub struct PluginRuntime {
    processes: BTreeMap<String, ManagedPluginProcess>,
    event_tx: Option<Sender<PluginRuntimeEvent>>,
    event_rx: Option<Receiver<PluginRuntimeEvent>>,
    next_request_id: u64,
    pending_requests: BTreeMap<(String, u64), PendingPluginRequest>,
    pending_events: VecDeque<PluginRuntimeEvent>,
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self {
            processes: BTreeMap::new(),
            event_tx: None,
            event_rx: None,
            next_request_id: FIRST_COMMAND_REQUEST_ID,
            pending_requests: BTreeMap::new(),
            pending_events: VecDeque::new(),
        }
    }
}

impl PluginRuntime {
    /// Starts process-backed plugins declared by a loaded plugin registry.
    pub fn start_from_registry(plugins: &PluginRegistry) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let mut runtime = Self {
            processes: BTreeMap::new(),
            event_tx: Some(event_tx.clone()),
            event_rx: Some(event_rx),
            next_request_id: FIRST_COMMAND_REQUEST_ID,
            pending_requests: BTreeMap::new(),
            pending_events: VecDeque::new(),
        };

        for (plugin_name, plugin) in plugins.iter() {
            let Some(process) = plugin.process() else {
                runtime.processes.insert(
                    plugin_name.to_string(),
                    ManagedPluginProcess {
                        runtime: None,
                        reader: None,
                        state: PluginProcessState::NotConfigured,
                        capabilities: Vec::new(),
                    },
                );
                continue;
            };

            let mut managed = ManagedPluginProcess {
                runtime: None,
                reader: None,
                state: PluginProcessState::Starting,
                capabilities: Vec::new(),
            };

            match PluginProcessRuntime::spawn_in(process, plugin.root()).and_then(
                |mut process_runtime| {
                    let capabilities = initialize_plugin_process(
                        plugin_name,
                        plugin.manifest.version.as_str(),
                        &mut process_runtime,
                    )?;
                    Ok((process_runtime, capabilities))
                },
            ) {
                Ok((mut process_runtime, capabilities)) => {
                    match process_runtime.take_stdout() {
                        Ok(stdout) => {
                            managed.reader = Some(spawn_reader_thread(
                                plugin_name.to_string(),
                                stdout,
                                event_tx.clone(),
                            ));
                        }
                        Err(error) => {
                            managed.state = PluginProcessState::Failed(error.to_string());
                            runtime.processes.insert(plugin_name.to_string(), managed);
                            continue;
                        }
                    }
                    managed.runtime = Some(process_runtime);
                    managed.state = PluginProcessState::Running;
                    managed.capabilities = capabilities;
                    tracing::debug!(plugin = plugin_name, "started plugin process");
                }
                Err(error) => {
                    let message = error.to_string();
                    tracing::warn!(plugin = plugin_name, error = %message, "failed to start plugin process");
                    managed.state = PluginProcessState::Failed(message);
                }
            }

            runtime.processes.insert(plugin_name.to_string(), managed);
        }

        runtime
    }

    /// Returns process status for a plugin.
    pub fn status(&self, plugin: &str) -> Option<PluginProcessStatus> {
        self.processes
            .get(plugin)
            .map(|process| PluginProcessStatus {
                plugin: plugin.to_string(),
                state: process.state.clone(),
            })
    }

    /// Returns all process statuses in plugin-name order.
    pub fn statuses(&self) -> Vec<PluginProcessStatus> {
        self.processes
            .iter()
            .map(|(plugin, process)| PluginProcessStatus {
                plugin: plugin.clone(),
                state: process.state.clone(),
            })
            .collect()
    }

    /// Returns user-facing status entries in plugin-name order.
    pub fn status_entries(&self) -> Vec<PluginStatusEntry> {
        self.processes
            .iter()
            .map(|(plugin, process)| PluginStatusEntry {
                plugin: plugin.clone(),
                state: process.state.clone(),
                capabilities: process.capabilities.clone(),
                error: match &process.state {
                    PluginProcessState::Failed(error) => Some(error.clone()),
                    PluginProcessState::NotConfigured
                    | PluginProcessState::Starting
                    | PluginProcessState::Running
                    | PluginProcessState::Stopped => None,
                },
            })
            .collect()
    }

    /// Returns failed process statuses in plugin-name order.
    pub fn failures(&self) -> Vec<PluginProcessStatus> {
        self.statuses()
            .into_iter()
            .filter(|status| matches!(status.state, PluginProcessState::Failed(_)))
            .collect()
    }

    /// Sends a message to a running plugin process.
    pub fn send(&mut self, plugin: &str, message: &PluginMessage) -> Result<(), PluginLoadError> {
        let process = self
            .processes
            .get_mut(plugin)
            .ok_or_else(|| PluginLoadError::runtime(format!("unknown plugin process {plugin}")))?;
        let runtime = process.runtime.as_mut().ok_or_else(|| {
            PluginLoadError::runtime(format!("plugin process {plugin} is not running"))
        })?;
        runtime.send(message)
    }

    /// Returns true when a running plugin advertised a capability.
    pub fn has_capability(&self, plugin: &str, capability: &str) -> bool {
        self.processes
            .get(plugin)
            .is_some_and(|process| process.capabilities.iter().any(|item| item == capability))
    }

    /// Returns advertised capabilities for a plugin.
    pub fn capabilities(&self, plugin: &str) -> Option<&[String]> {
        self.processes
            .get(plugin)
            .map(|process| process.capabilities.as_slice())
    }

    /// Sends a response to a request received from a plugin process.
    pub fn send_response(
        &mut self,
        plugin: &str,
        response: PluginResponse,
    ) -> Result<(), PluginLoadError> {
        self.send(plugin, &PluginMessage::Response(response))
    }

    /// Sends a request to a running plugin process and returns the allocated id.
    pub fn send_request(
        &mut self,
        plugin: &str,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> Result<u64, PluginLoadError> {
        self.send_request_with_timeout(plugin, method, params, DEFAULT_REQUEST_TIMEOUT)
    }

    /// Sends a request with an explicit timeout and returns the allocated id.
    pub fn send_request_with_timeout(
        &mut self,
        plugin: &str,
        method: impl Into<String>,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<u64, PluginLoadError> {
        let id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .saturating_add(1)
            .max(FIRST_COMMAND_REQUEST_ID);
        let method = method.into();
        if !self.has_capability(plugin, method.as_str()) {
            return Err(PluginLoadError::runtime(format!(
                "plugin process {plugin} did not advertise capability {method}"
            )));
        }
        self.send(
            plugin,
            &PluginMessage::Request(PluginRequest::new(id, method.clone(), params)),
        )?;
        self.pending_requests.insert(
            (plugin.to_string(), id),
            PendingPluginRequest {
                plugin: plugin.to_string(),
                id,
                method,
                started_at: Instant::now(),
                timeout,
            },
        );
        Ok(id)
    }

    /// Returns the number of currently pending requests.
    #[cfg(test)]
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }

    /// Polls the next pending runtime event.
    pub fn poll_event(&mut self) -> Option<PluginRuntimeEvent> {
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }

        if let Some(event) = self.event_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.handle_runtime_event_state(&event);
            return Some(event);
        }

        self.poll_timeout_event()
    }

    fn handle_runtime_event_state(&mut self, event: &PluginRuntimeEvent) {
        match &event {
            PluginRuntimeEvent::ProcessExited { plugin } => {
                if let Some(process) = self.processes.get_mut(plugin)
                    && process.runtime.is_some()
                {
                    process.state = PluginProcessState::Stopped;
                }
                self.fail_pending_requests_for_plugin(plugin, "plugin process exited".to_string());
            }
            PluginRuntimeEvent::ProtocolError { plugin, error }
            | PluginRuntimeEvent::RuntimeError { plugin, error } => {
                if let Some(process) = self.processes.get_mut(plugin) {
                    process.state = PluginProcessState::Failed(error.clone());
                }
                self.fail_pending_requests_for_plugin(plugin, error.clone());
            }
            PluginRuntimeEvent::ResponseReceived { plugin, response } => {
                if self
                    .pending_requests
                    .remove(&(plugin.clone(), response.id))
                    .is_none()
                {
                    tracing::debug!(
                        plugin,
                        id = response.id,
                        "ignoring response for unknown plugin request id"
                    );
                }
            }
            PluginRuntimeEvent::RequestReceived { .. }
            | PluginRuntimeEvent::NotificationReceived { .. }
            | PluginRuntimeEvent::RequestTimedOut { .. }
            | PluginRuntimeEvent::RequestFailed { .. } => {}
        }
    }

    fn poll_timeout_event(&mut self) -> Option<PluginRuntimeEvent> {
        let now = Instant::now();
        let key = self
            .pending_requests
            .iter()
            .find(|(_, request)| request.timed_out(now))
            .map(|(key, _)| key.clone())?;
        let request = self.pending_requests.remove(&key)?;
        Some(PluginRuntimeEvent::RequestTimedOut {
            plugin: request.plugin,
            id: request.id,
            method: request.method,
        })
    }

    fn fail_pending_requests_for_plugin(&mut self, plugin: &str, error: String) {
        let keys = self
            .pending_requests
            .keys()
            .filter(|(request_plugin, _)| request_plugin == plugin)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(request) = self.pending_requests.remove(&key) {
                self.pending_events
                    .push_back(PluginRuntimeEvent::RequestFailed {
                        plugin: request.plugin,
                        id: request.id,
                        method: request.method,
                        error: error.clone(),
                    });
            }
        }
    }

    /// Stops all running plugin processes.
    pub fn shutdown(&mut self) {
        self.event_tx.take();
        for process in self.processes.values_mut() {
            if let Some(runtime) = process.runtime.take() {
                runtime.shutdown();
                process.state = PluginProcessState::Stopped;
            }
            if let Some(reader) = process.reader.take() {
                reader.join().ok();
            }
        }
    }
}

fn spawn_reader_thread<R: Read + Send + 'static>(
    plugin: String,
    reader: R,
    event_tx: Sender<PluginRuntimeEvent>,
) -> JoinHandle<()> {
    thread::spawn(move || run_reader_loop(plugin, reader, event_tx))
}

fn run_reader_loop<R: Read>(plugin: String, reader: R, event_tx: Sender<PluginRuntimeEvent>) {
    let mut reader = BufReader::new(reader);
    loop {
        match read_frame(&mut reader) {
            Ok(PluginMessage::Response(response)) => {
                event_tx
                    .send(PluginRuntimeEvent::ResponseReceived {
                        plugin: plugin.clone(),
                        response,
                    })
                    .ok();
            }
            Ok(PluginMessage::Notification(notification)) => {
                event_tx
                    .send(PluginRuntimeEvent::NotificationReceived {
                        plugin: plugin.clone(),
                        notification,
                    })
                    .ok();
            }
            Ok(PluginMessage::Request(request)) => {
                event_tx
                    .send(PluginRuntimeEvent::RequestReceived {
                        plugin: plugin.clone(),
                        request,
                    })
                    .ok();
            }
            Err(PluginLoadError::Protocol { message }) => {
                event_tx
                    .send(PluginRuntimeEvent::ProtocolError {
                        plugin: plugin.clone(),
                        error: message,
                    })
                    .ok();
                break;
            }
            Err(error) => {
                if error.to_string().contains("failed to fill whole buffer") {
                    event_tx
                        .send(PluginRuntimeEvent::ProcessExited {
                            plugin: plugin.clone(),
                        })
                        .ok();
                } else {
                    event_tx
                        .send(PluginRuntimeEvent::RuntimeError {
                            plugin: plugin.clone(),
                            error: error.to_string(),
                        })
                        .ok();
                }
                break;
            }
        }
    }
}

impl Drop for PluginRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn initialize_plugin_process(
    plugin_name: &str,
    plugin_version: &str,
    runtime: &mut PluginProcessRuntime,
) -> Result<Vec<String>, PluginLoadError> {
    let request = PluginMessage::Request(PluginRequest::new(
        INITIALIZE_REQUEST_ID,
        "editor/initialize",
        json!({
            "protocol_version": PROTOCOL_VERSION,
            "editor": {
                "name": "urvim",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "plugin": {
                "name": plugin_name,
                "version": plugin_version,
            },
            "capabilities": EDITOR_CAPABILITIES,
        }),
    ));
    runtime.send(&request)?;
    match runtime.recv()? {
        PluginMessage::Response(response)
            if response.id == INITIALIZE_REQUEST_ID && response.error.is_none() =>
        {
            parse_initialize_response(plugin_name, &response)
        }
        PluginMessage::Response(response) if response.id == INITIALIZE_REQUEST_ID => {
            Err(PluginLoadError::runtime(format!(
                "plugin initialize failed: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            )))
        }
        other => Err(PluginLoadError::runtime(format!(
            "unexpected plugin initialize response: {other:?}"
        ))),
    }
}

fn parse_initialize_response(
    plugin_name: &str,
    response: &PluginResponse,
) -> Result<Vec<String>, PluginLoadError> {
    let result = response
        .result
        .as_ref()
        .ok_or_else(|| PluginLoadError::runtime("plugin initialize response missing result"))?;
    let protocol_version = result
        .get("protocol_version")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            PluginLoadError::runtime("plugin initialize response missing protocol_version")
        })?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(PluginLoadError::runtime(format!(
            "plugin protocol version {protocol_version} is not supported"
        )));
    }

    let capabilities = result
        .get("capabilities")
        .and_then(|value| value.as_array())
        .ok_or_else(|| PluginLoadError::runtime("plugin initialize response missing capabilities"))?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                PluginLoadError::runtime("plugin initialize capabilities must be strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    tracing::debug!(plugin = plugin_name, capabilities = ?capabilities, "initialized plugin process");
    Ok(capabilities)
}

impl Drop for PluginProcessRuntime {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use crate::globals;
    use crate::notification::NotificationLevel;
    use crate::plugin::{MANIFEST_FILE_NAME, PluginNotification, PluginRegistry};
    use crate::plugin::{PluginRequest, PluginResponse, encode_frame};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock, mpsc};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn notification_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "urvim-plugin-runtime-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_manifest(root: &Path, contents: &str) {
        std::fs::create_dir_all(root).expect("plugin root should be created");
        std::fs::write(root.join(MANIFEST_FILE_NAME), contents).expect("manifest should write");
    }

    fn config_for_plugin(name: &str, path: PathBuf, enabled: bool) -> Config {
        Config {
            plugins: BTreeMap::from([(name.to_string(), PluginConfig { enabled, path })]),
            ..Config::default()
        }
    }

    fn command_available(command: &str) -> bool {
        Command::new(command).arg("--version").output().is_ok()
    }

    fn wait_for_event(runtime: &mut PluginRuntime) -> PluginRuntimeEvent {
        for _ in 0..100 {
            if let Some(event) = runtime.poll_event() {
                return event;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for plugin runtime event");
    }

    fn test_runtime_with_event_sender() -> (PluginRuntime, Sender<PluginRuntimeEvent>) {
        let (event_tx, event_rx) = mpsc::channel();
        let runtime = PluginRuntime {
            processes: BTreeMap::from([(
                "demo-plugin".to_string(),
                ManagedPluginProcess {
                    runtime: None,
                    reader: None,
                    state: PluginProcessState::Running,
                    capabilities: vec!["demo/first".to_string(), "demo/second".to_string()],
                },
            )]),
            event_tx: Some(event_tx.clone()),
            event_rx: Some(event_rx),
            next_request_id: FIRST_COMMAND_REQUEST_ID,
            pending_requests: BTreeMap::new(),
            pending_events: VecDeque::new(),
        };
        (runtime, event_tx)
    }

    fn insert_pending_request(
        runtime: &mut PluginRuntime,
        plugin: &str,
        id: u64,
        method: &str,
        started_at: Instant,
        timeout: Duration,
    ) {
        runtime.pending_requests.insert(
            (plugin.to_string(), id),
            PendingPluginRequest {
                plugin: plugin.to_string(),
                id,
                method: method.to_string(),
                started_at,
                timeout,
            },
        );
    }

    #[test]
    fn runtime_can_spawn_process_and_exchange_message() {
        let process = PluginProcess {
            command: "cat".to_string(),
            args: Vec::new(),
            env: Default::default(),
        };
        let request = PluginMessage::Request(PluginRequest::new(1, "editor/initialize", json!({})));
        let mut runtime = PluginProcessRuntime::spawn(&process).expect("process should start");

        runtime.send(&request).expect("message should send");
        let response = runtime.recv().expect("message should echo");

        assert_eq!(response, request);
        runtime.shutdown();
    }

    #[test]
    fn process_failure_does_not_remove_manifest_contributions() {
        let manifest = crate::plugin::PluginManifest::parse_from_str(
            "test",
            r#"
name = "demo"
version = "0.1.0"
themes = ["themes/demo.toml"]

[process]
command = "/definitely/missing/urvim-plugin"

[scripts]
wq = ["write", "quit"]
"#,
            "/plugins/demo",
        )
        .expect("manifest should parse");

        let error = PluginProcessRuntime::spawn(manifest.process.as_ref().unwrap())
            .expect_err("process should fail");

        assert!(error.to_string().contains("runtime"));
        assert_eq!(manifest.themes.len(), 1);
        assert!(manifest.scripts.contains_key("wq"));
    }

    #[test]
    fn response_constructor_supports_error_payload() {
        let response = PluginResponse::error(7, "boom");

        assert_eq!(response.id, 7);
        assert_eq!(response.error.as_deref(), Some("boom"));
    }

    #[test]
    fn initialize_response_accepts_matching_protocol() {
        let response = PluginResponse::success(
            INITIALIZE_REQUEST_ID,
            json!({
                "protocol_version": 1,
                "capabilities": ["demo/echo"],
            }),
        );

        let capabilities = parse_initialize_response("demo-plugin", &response)
            .expect("matching protocol should initialize");

        assert_eq!(capabilities, vec!["demo/echo".to_string()]);
    }

    #[test]
    fn initialize_response_rejects_unsupported_protocol() {
        let response = PluginResponse::success(
            INITIALIZE_REQUEST_ID,
            json!({
                "protocol_version": 99,
                "capabilities": ["demo/echo"],
            }),
        );

        let error = parse_initialize_response("demo-plugin", &response)
            .expect_err("unsupported protocol should fail");

        assert!(error.to_string().contains("not supported"));
    }

    #[test]
    fn request_send_fails_when_capability_is_missing() {
        let (mut runtime, _event_tx) = test_runtime_with_event_sender();

        let error = runtime
            .send_request("demo-plugin", "demo/missing", json!({}))
            .expect_err("missing capability should fail");

        assert!(error.to_string().contains("did not advertise capability"));
        assert_eq!(runtime.pending_request_count(), 0);
    }

    #[test]
    fn status_model_includes_running_plugins() {
        let (runtime, _event_tx) = test_runtime_with_event_sender();

        let statuses = runtime.status_entries();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].plugin, "demo-plugin");
        assert_eq!(statuses[0].state, PluginProcessState::Running);
        assert_eq!(
            statuses[0].capabilities,
            vec!["demo/first".to_string(), "demo/second".to_string()]
        );
        assert_eq!(statuses[0].error, None);
    }

    #[test]
    fn status_model_includes_failed_plugin_errors() {
        let mut runtime = PluginRuntime::default();
        runtime.processes.insert(
            "failed-plugin".to_string(),
            ManagedPluginProcess {
                runtime: None,
                reader: None,
                state: PluginProcessState::Failed("boom".to_string()),
                capabilities: Vec::new(),
            },
        );

        let statuses = runtime.status_entries();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].plugin, "failed-plugin");
        assert_eq!(statuses[0].error.as_deref(), Some("boom"));
    }

    #[test]
    fn manager_rejects_process_without_initialize_response() {
        let root = unique_temp_dir("cat");
        write_manifest(
            &root,
            r#"
name = "cat-plugin"
version = "0.1.0"

[process]
command = "cat"
"#,
        );
        let registry =
            PluginRegistry::load_from_config(&config_for_plugin("cat-plugin", root.clone(), true))
                .expect("plugin should load");

        let mut runtime = PluginRuntime::start_from_registry(&registry);

        assert!(matches!(
            runtime.status("cat-plugin").map(|status| status.state),
            Some(PluginProcessState::Failed(_))
        ));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn pending_requests_complete_out_of_order() {
        let (mut runtime, event_tx) = test_runtime_with_event_sender();
        let now = Instant::now();
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            2,
            "demo/first",
            now,
            Duration::from_secs(10),
        );
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            3,
            "demo/second",
            now,
            Duration::from_secs(10),
        );

        event_tx
            .send(PluginRuntimeEvent::ResponseReceived {
                plugin: "demo-plugin".to_string(),
                response: PluginResponse::success(3, json!({ "ok": 2 })),
            })
            .expect("event should send");
        event_tx
            .send(PluginRuntimeEvent::ResponseReceived {
                plugin: "demo-plugin".to_string(),
                response: PluginResponse::success(2, json!({ "ok": 1 })),
            })
            .expect("event should send");

        assert!(matches!(
            runtime.poll_event(),
            Some(PluginRuntimeEvent::ResponseReceived { response, .. }) if response.id == 3
        ));
        assert_eq!(runtime.pending_request_count(), 1);
        assert!(matches!(
            runtime.poll_event(),
            Some(PluginRuntimeEvent::ResponseReceived { response, .. }) if response.id == 2
        ));
        assert_eq!(runtime.pending_request_count(), 0);
    }

    #[test]
    fn unknown_response_id_is_ignored() {
        let (mut runtime, event_tx) = test_runtime_with_event_sender();
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            2,
            "demo/known",
            Instant::now(),
            Duration::from_secs(10),
        );

        event_tx
            .send(PluginRuntimeEvent::ResponseReceived {
                plugin: "demo-plugin".to_string(),
                response: PluginResponse::success(99, json!({})),
            })
            .expect("event should send");

        assert!(matches!(
            runtime.poll_event(),
            Some(PluginRuntimeEvent::ResponseReceived { response, .. }) if response.id == 99
        ));
        assert_eq!(runtime.pending_request_count(), 1);
    }

    #[test]
    fn timeout_fails_pending_request() {
        let (mut runtime, _event_tx) = test_runtime_with_event_sender();
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            2,
            "demo/slow",
            Instant::now() - Duration::from_secs(2),
            Duration::from_millis(1),
        );

        assert!(matches!(
            runtime.poll_event(),
            Some(PluginRuntimeEvent::RequestTimedOut { plugin, id, method })
                if plugin == "demo-plugin" && id == 2 && method == "demo/slow"
        ));
        assert_eq!(runtime.pending_request_count(), 0);
    }

    #[test]
    fn process_exit_fails_pending_requests() {
        let (mut runtime, event_tx) = test_runtime_with_event_sender();
        let now = Instant::now();
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            2,
            "demo/first",
            now,
            Duration::from_secs(10),
        );
        insert_pending_request(
            &mut runtime,
            "demo-plugin",
            3,
            "demo/second",
            now,
            Duration::from_secs(10),
        );

        event_tx
            .send(PluginRuntimeEvent::ProcessExited {
                plugin: "demo-plugin".to_string(),
            })
            .expect("event should send");

        assert!(matches!(
            runtime.poll_event(),
            Some(PluginRuntimeEvent::ProcessExited { plugin }) if plugin == "demo-plugin"
        ));
        assert_eq!(runtime.pending_request_count(), 0);

        let first = runtime.poll_event();
        let second = runtime.poll_event();
        let mut ids = vec![
            match first {
                Some(PluginRuntimeEvent::RequestFailed { id, .. }) => id,
                other => panic!("unexpected event: {other:?}"),
            },
            match second {
                Some(PluginRuntimeEvent::RequestFailed { id, .. }) => id,
                other => panic!("unexpected event: {other:?}"),
            },
        ];
        ids.sort_unstable();
        assert_eq!(ids, vec![2, 3]);
    }

    #[test]
    fn manager_sets_plugin_root_as_current_dir() {
        let root = unique_temp_dir("cwd");
        std::fs::create_dir_all(&root).expect("plugin root should be created");
        std::fs::write(root.join("marker"), "ok").expect("marker should write");
        write_manifest(
            &root,
            r#"
name = "cwd-plugin"
version = "0.1.0"

[process]
command = "sh"
args = ["-c", "test -f marker && cat"]
"#,
        );
        let registry =
            PluginRegistry::load_from_config(&config_for_plugin("cwd-plugin", root.clone(), true))
                .expect("plugin should load");

        let mut runtime = PluginRuntime::start_from_registry(&registry);

        assert!(matches!(
            runtime.status("cwd-plugin").map(|status| status.state),
            Some(PluginProcessState::Failed(_))
        ));
        runtime.shutdown();

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn manager_marks_missing_process_failed_without_removing_contributions() {
        let root = unique_temp_dir("missing");
        write_manifest(
            &root,
            r#"
name = "missing-plugin"
version = "0.1.0"
themes = ["themes/demo.toml"]

[process]
command = "/definitely/missing/urvim-plugin"

[scripts]
wq = ["write", "quit"]
"#,
        );
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "missing-plugin",
            root.clone(),
            true,
        ))
        .expect("plugin should load");

        let runtime = PluginRuntime::start_from_registry(&registry);

        assert!(matches!(
            runtime.status("missing-plugin").map(|status| status.state),
            Some(PluginProcessState::Failed(_))
        ));
        assert_eq!(
            registry
                .script("missing-plugin", "wq")
                .map(|script| script.len()),
            Some(2)
        );
        assert_eq!(
            registry
                .get("missing-plugin")
                .map(|plugin| plugin.themes().len()),
            Some(1)
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn manager_records_not_configured_plugins() {
        let root = unique_temp_dir("static");
        write_manifest(
            &root,
            r#"
name = "static-plugin"
version = "0.1.0"
"#,
        );
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "static-plugin",
            root.clone(),
            true,
        ))
        .expect("plugin should load");

        let runtime = PluginRuntime::start_from_registry(&registry);

        assert_eq!(
            runtime.status("static-plugin").map(|status| status.state),
            Some(PluginProcessState::NotConfigured)
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn disabled_plugin_is_not_started() {
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "disabled-plugin",
            PathBuf::from("/definitely/missing/disabled-plugin"),
            false,
        ))
        .expect("disabled plugin should skip IO");

        let runtime = PluginRuntime::start_from_registry(&registry);

        assert!(runtime.statuses().is_empty());
    }

    #[test]
    fn manager_starts_demo_plugin_when_uv_is_available() {
        if !command_available("uv") {
            return;
        }
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "demo-plugin",
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/plugins/demo-plugin"
            )),
            true,
        ))
        .expect("demo plugin should load");

        let mut runtime = PluginRuntime::start_from_registry(&registry);

        assert_eq!(
            runtime.status("demo-plugin").map(|status| status.state),
            Some(PluginProcessState::Running)
        );
        assert_eq!(
            runtime.capabilities("demo-plugin"),
            Some(&["demo/echo".to_string()][..])
        );
        runtime.shutdown();
    }

    #[test]
    fn runtime_receives_response_asynchronously() {
        let root = unique_temp_dir("response-event");
        write_manifest(
            &root,
            r#"
name = "response-plugin"
version = "0.1.0"

[process]
command = "cat"
"#,
        );
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "response-plugin",
            root.clone(),
            true,
        ))
        .expect("plugin should load");
        let mut runtime = PluginRuntime::start_from_registry(&registry);
        if !matches!(
            runtime.status("notify-plugin").map(|status| status.state),
            Some(PluginProcessState::Running)
        ) {
            std::fs::remove_dir_all(root).ok();
            return;
        }
        let message = PluginMessage::Response(PluginResponse::success(42, json!({"ok": true})));

        runtime
            .send("response-plugin", &message)
            .expect("message should send");
        let event = wait_for_event(&mut runtime);

        assert!(matches!(
            event,
            PluginRuntimeEvent::ResponseReceived { plugin, response }
                if plugin == "response-plugin" && response.id == 42
        ));
        runtime.shutdown();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn runtime_receives_notification_asynchronously() {
        let root = unique_temp_dir("notification-event");
        write_manifest(
            &root,
            r#"
name = "notify-plugin"
version = "0.1.0"

[process]
command = "cat"
"#,
        );
        let registry = PluginRegistry::load_from_config(&config_for_plugin(
            "notify-plugin",
            root.clone(),
            true,
        ))
        .expect("plugin should load");
        let mut runtime = PluginRuntime::start_from_registry(&registry);
        if !matches!(
            runtime.status("notify-plugin").map(|status| status.state),
            Some(PluginProcessState::Running)
        ) {
            std::fs::remove_dir_all(root).ok();
            return;
        }
        let message = PluginMessage::Notification(PluginNotification::new(
            "editor/notify",
            json!({"message": "hello"}),
        ));

        runtime
            .send("notify-plugin", &message)
            .expect("message should send");
        let event = wait_for_event(&mut runtime);

        assert!(matches!(
            event,
            PluginRuntimeEvent::NotificationReceived { plugin, notification }
                if plugin == "notify-plugin" && notification.method == "editor/notify"
        ));
        runtime.shutdown();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reader_reports_process_exit() {
        let (tx, rx) = mpsc::channel();
        run_reader_loop("exit-plugin".to_string(), Cursor::new(Vec::<u8>::new()), tx);

        let event = rx.recv().expect("event should be sent");

        assert_eq!(
            event,
            PluginRuntimeEvent::ProcessExited {
                plugin: "exit-plugin".to_string()
            }
        );
    }

    #[test]
    fn reader_reports_bad_messagepack_as_protocol_error() {
        let (tx, rx) = mpsc::channel();
        let frame = vec![0, 0, 0, 1, 0xff];

        run_reader_loop("bad-plugin".to_string(), Cursor::new(frame), tx);

        let event = rx.recv().expect("event should be sent");

        assert!(matches!(
            event,
            PluginRuntimeEvent::ProtocolError { plugin, .. } if plugin == "bad-plugin"
        ));
    }

    #[test]
    fn reader_decodes_valid_frame() {
        let (tx, rx) = mpsc::channel();
        let message = PluginMessage::Notification(PluginNotification::new("demo/event", json!({})));
        let frame = encode_frame(&message).expect("message should encode");

        run_reader_loop("frame-plugin".to_string(), Cursor::new(frame), tx);

        let event = rx.recv().expect("event should be sent");

        assert!(matches!(
            event,
            PluginRuntimeEvent::NotificationReceived { plugin, notification }
                if plugin == "frame-plugin" && notification.method == "demo/event"
        ));
    }

    #[test]
    fn editor_notify_info_maps_to_queue() {
        let _guard = notification_test_lock();
        globals::clear_notifications();
        let event = PluginRuntimeEvent::NotificationReceived {
            plugin: "demo-plugin".to_string(),
            notification: PluginNotification::new(
                "editor/notify",
                json!({"level": "info", "message": "hello"}),
            ),
        };

        assert!(handle_runtime_event(&event));
        let active = globals::active_notification(std::time::Instant::now()).expect("notification");

        assert_eq!(active.level, NotificationLevel::Info);
        assert_eq!(active.text, "demo-plugin: hello");
        globals::clear_notifications();
    }

    #[test]
    fn editor_notify_warn_and_error_levels_map_to_queue() {
        let _guard = notification_test_lock();
        for (level, expected) in [
            ("warn", NotificationLevel::Warn),
            ("warning", NotificationLevel::Warn),
            ("error", NotificationLevel::Error),
        ] {
            globals::clear_notifications();
            let event = PluginRuntimeEvent::NotificationReceived {
                plugin: "demo-plugin".to_string(),
                notification: PluginNotification::new(
                    "editor/notify",
                    json!({"level": level, "message": "hello"}),
                ),
            };

            assert!(handle_runtime_event(&event));
            let active =
                globals::active_notification(std::time::Instant::now()).expect("notification");
            assert_eq!(active.level, expected);
        }
        globals::clear_notifications();
    }

    #[test]
    fn editor_notify_invalid_level_downgrades_to_warning() {
        let _guard = notification_test_lock();
        globals::clear_notifications();
        let event = PluginRuntimeEvent::NotificationReceived {
            plugin: "demo-plugin".to_string(),
            notification: PluginNotification::new(
                "editor/notify",
                json!({"level": "loud", "message": "hello"}),
            ),
        };

        assert!(handle_runtime_event(&event));
        let active = globals::active_notification(std::time::Instant::now()).expect("notification");

        assert_eq!(active.level, NotificationLevel::Warn);
        globals::clear_notifications();
    }

    #[test]
    fn unknown_plugin_notice_method_is_ignored() {
        let _guard = notification_test_lock();
        globals::clear_notifications();
        let event = PluginRuntimeEvent::NotificationReceived {
            plugin: "demo-plugin".to_string(),
            notification: PluginNotification::new("demo/unknown", json!({"message": "hello"})),
        };

        assert!(!handle_runtime_event(&event));
        assert!(globals::active_notification(std::time::Instant::now()).is_none());
    }
}
