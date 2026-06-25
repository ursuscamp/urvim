use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bearscript::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plugin) enum PluginJobStatus {
    Running,
    Exited,
    Failed,
    Killed,
    TimedOut,
}

impl PluginJobStatus {
    pub(in crate::plugin) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Killed => "killed",
            Self::TimedOut => "timed_out",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::plugin) enum PluginJobEvent {
    Stdout {
        job_id: u64,
        text: String,
    },
    Stderr {
        job_id: u64,
        text: String,
    },
    Exit {
        job_id: u64,
        status: PluginJobStatus,
        code: Option<i32>,
    },
}

#[derive(Clone)]
pub(in crate::plugin) struct PluginJobCallbacks {
    pub(in crate::plugin) on_stdout: Option<Value>,
    pub(in crate::plugin) on_stderr: Option<Value>,
    pub(in crate::plugin) on_exit: Option<Value>,
}

pub(in crate::plugin) struct PluginJobSpawn {
    pub(in crate::plugin) id: u64,
    pub(in crate::plugin) callbacks: PluginJobCallbacks,
}

pub(in crate::plugin) struct PluginJobRegistry {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<u64, PluginJob>>,
    event_tx: Sender<PluginJobEvent>,
    event_rx: Mutex<Receiver<PluginJobEvent>>,
}

struct PluginJob {
    plugin: String,
    cmd: String,
    status: PluginJobStatus,
    child: Arc<Mutex<Child>>,
    stdin: Option<ChildStdin>,
    kill_requested: Arc<Mutex<Option<PluginJobStatus>>>,
}

struct PluginJobSpec {
    cmd: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    stdin: Option<String>,
    timeout_ms: Option<u64>,
    callbacks: PluginJobCallbacks,
}

impl Default for PluginJobRegistry {
    fn default() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
            event_tx,
            event_rx: Mutex::new(event_rx),
        }
    }
}

impl PluginJobRegistry {
    pub(in crate::plugin) fn spawn(
        &self,
        plugin: &str,
        opts: Value,
    ) -> Result<PluginJobSpawn, String> {
        let spec = PluginJobSpec::from_value(opts)?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut command = Command::new(&spec.cmd);
        command
            .args(&spec.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        for (key, value) in &spec.env {
            command.env(key, value);
        }
        command.stdin(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to spawn job {id}: {error}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();
        if let Some(input) = spec.stdin {
            if let Some(writer) = stdin.as_mut() {
                writer.write_all(input.as_bytes()).map_err(|error| {
                    format!("failed to write initial stdin for job {id}: {error}")
                })?;
            }
        }
        let child = Arc::new(Mutex::new(child));
        let kill_requested = Arc::new(Mutex::new(None));
        self.jobs.lock().expect("job registry poisoned").insert(
            id,
            PluginJob {
                plugin: plugin.to_string(),
                cmd: spec.cmd.clone(),
                status: PluginJobStatus::Running,
                child: Arc::clone(&child),
                stdin,
                kill_requested: Arc::clone(&kill_requested),
            },
        );

        if let Some(stdout) = stdout {
            spawn_output_reader(id, stdout, self.event_tx.clone(), OutputStream::Stdout);
        }
        if let Some(stderr) = stderr {
            spawn_output_reader(id, stderr, self.event_tx.clone(), OutputStream::Stderr);
        }
        spawn_exit_watcher(
            id,
            Arc::clone(&child),
            Arc::clone(&kill_requested),
            self.event_tx.clone(),
        );
        if let Some(timeout_ms) = spec.timeout_ms {
            spawn_timeout_watcher(id, child, kill_requested, timeout_ms);
        }

        Ok(PluginJobSpawn {
            id,
            callbacks: spec.callbacks,
        })
    }

    pub(in crate::plugin) fn kill(&self, job_id: u64) -> Result<(), String> {
        let jobs = self.jobs.lock().expect("job registry poisoned");
        let job = jobs
            .get(&job_id)
            .ok_or_else(|| format!("unknown job_id {job_id}"))?;
        if job.status != PluginJobStatus::Running {
            return Ok(());
        }
        *job.kill_requested.lock().expect("job status poisoned") = Some(PluginJobStatus::Killed);
        job.child
            .lock()
            .expect("child process poisoned")
            .kill()
            .map_err(|error| format!("failed to kill job {job_id}: {error}"))
    }

    pub(in crate::plugin) fn status(&self, job_id: u64) -> Result<PluginJobStatus, String> {
        let jobs = self.jobs.lock().expect("job registry poisoned");
        jobs.get(&job_id)
            .map(|job| job.status)
            .ok_or_else(|| format!("unknown job_id {job_id}"))
    }

    pub(in crate::plugin) fn write_stdin(&self, job_id: u64, text: &str) -> Result<(), String> {
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| format!("unknown job_id {job_id}"))?;
        if job.status != PluginJobStatus::Running {
            return Err(format!("job_id {job_id} is not running"));
        }
        let stdin = job
            .stdin
            .as_mut()
            .ok_or_else(|| format!("job_id {job_id} stdin is closed"))?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed to write stdin for job {job_id}: {error}"))
    }

    pub(in crate::plugin) fn close_stdin(&self, job_id: u64) -> Result<(), String> {
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| format!("unknown job_id {job_id}"))?;
        job.stdin = None;
        Ok(())
    }

    pub(in crate::plugin) fn list(&self) -> Vec<Value> {
        let jobs = self.jobs.lock().expect("job registry poisoned");
        jobs.iter()
            .map(|(id, job)| {
                Value::Map(
                    HashMap::from([
                        ("id".to_string(), Value::Number(*id as f64)),
                        (
                            "plugin".to_string(),
                            Value::String(job.plugin.clone().into_boxed_str().into()),
                        ),
                        (
                            "cmd".to_string(),
                            Value::String(job.cmd.clone().into_boxed_str().into()),
                        ),
                        (
                            "status".to_string(),
                            Value::String(job.status.as_str().into()),
                        ),
                    ])
                    .into(),
                )
            })
            .collect()
    }

    pub(in crate::plugin) fn poll_event(&self) -> Option<PluginJobEvent> {
        self.event_rx
            .lock()
            .expect("job event queue poisoned")
            .try_recv()
            .ok()
    }

    pub(in crate::plugin) fn mark_finished(
        &self,
        job_id: u64,
        status: PluginJobStatus,
    ) -> Option<String> {
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        let job = jobs.get_mut(&job_id)?;
        job.status = status;
        job.stdin = None;
        Some(job.plugin.clone())
    }

    pub(in crate::plugin) fn plugin_for_job(&self, job_id: u64) -> Option<String> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .get(&job_id)
            .map(|job| job.plugin.clone())
    }
}

impl PluginJobSpec {
    fn from_value(value: Value) -> Result<Self, String> {
        let Value::Map(mut map) = value else {
            return Err("jobs.spawn opts must be a map".to_string());
        };
        let cmd = required_string(&mut map, "cmd")?;
        let args = optional_string_list(&mut map, "args")?;
        let cwd = optional_string(&mut map, "cwd")?;
        let env = optional_env(&mut map)?;
        let stdin = optional_string(&mut map, "stdin")?;
        let timeout_ms = optional_u64(&mut map, "timeout_ms")?;
        let callbacks = PluginJobCallbacks {
            on_stdout: optional_callback(&mut map, "on_stdout")?,
            on_stderr: optional_callback(&mut map, "on_stderr")?,
            on_exit: optional_callback(&mut map, "on_exit")?,
        };
        Ok(Self {
            cmd,
            args,
            cwd,
            env,
            stdin,
            timeout_ms,
            callbacks,
        })
    }
}

fn required_string(map: &mut HashMap<String, Value>, key: &str) -> Result<String, String> {
    match map.remove(key) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value.to_string()),
        Some(_) => Err(format!("jobs.spawn {key} must be a non-empty string")),
        None => Err(format!("jobs.spawn requires {key}")),
    }
}

fn optional_string(map: &mut HashMap<String, Value>, key: &str) -> Result<Option<String>, String> {
    match map.remove(key) {
        Some(Value::String(value)) => Ok(Some(value.to_string())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(format!("jobs.spawn {key} must be a string")),
    }
}

fn optional_string_list(
    map: &mut HashMap<String, Value>,
    key: &str,
) -> Result<Vec<String>, String> {
    match map.remove(key) {
        Some(Value::List(items)) => items
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, item)| match item {
                Value::String(value) => Ok(value.to_string()),
                _ => Err(format!("jobs.spawn {key}[{index}] must be a string")),
            })
            .collect(),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err(format!("jobs.spawn {key} must be a list")),
    }
}

fn optional_env(map: &mut HashMap<String, Value>) -> Result<Vec<(String, String)>, String> {
    match map.remove("env") {
        Some(Value::Map(env)) => env
            .into_map()
            .into_iter()
            .map(|(key, value)| match value {
                Value::String(value) => Ok((key, value.to_string())),
                _ => Err(format!("jobs.spawn env.{key} must be a string")),
            })
            .collect(),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err("jobs.spawn env must be a map".to_string()),
    }
}

fn optional_u64(map: &mut HashMap<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match map.remove(key) {
        Some(Value::Number(value))
            if value.is_finite()
                && value >= 0.0
                && value.fract() == 0.0
                && value <= u64::MAX as f64 =>
        {
            Ok(Some(value as u64))
        }
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(format!("jobs.spawn {key} must be a non-negative integer")),
    }
}

fn optional_callback(map: &mut HashMap<String, Value>, key: &str) -> Result<Option<Value>, String> {
    match map.remove(key) {
        Some(callback @ (Value::ScriptFn(_) | Value::NativeFn(_))) => Ok(Some(callback)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(format!("jobs.spawn {key} must be a function")),
    }
}

enum OutputStream {
    Stdout,
    Stderr,
}

fn spawn_output_reader(
    job_id: u64,
    stream: impl Read + Send + 'static,
    event_tx: Sender<PluginJobEvent>,
    output_stream: OutputStream,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = [0; 8192];
        let mut pending = Vec::new();
        let mut last_flush = Instant::now();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    pending.extend_from_slice(&buffer[..read]);
                    if pending.len() >= 64 * 1024
                        || last_flush.elapsed() >= Duration::from_millis(16)
                    {
                        send_output_event(job_id, &event_tx, &output_stream, &mut pending);
                        last_flush = Instant::now();
                    }
                }
                Err(_) => break,
            }
        }
        send_output_event(job_id, &event_tx, &output_stream, &mut pending);
    });
}

fn send_output_event(
    job_id: u64,
    event_tx: &Sender<PluginJobEvent>,
    output_stream: &OutputStream,
    pending: &mut Vec<u8>,
) {
    if pending.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(pending).into_owned();
    pending.clear();
    match output_stream {
        OutputStream::Stdout => event_tx.send(PluginJobEvent::Stdout { job_id, text }).ok(),
        OutputStream::Stderr => event_tx.send(PluginJobEvent::Stderr { job_id, text }).ok(),
    };
}

fn spawn_exit_watcher(
    job_id: u64,
    child: Arc<Mutex<Child>>,
    kill_requested: Arc<Mutex<Option<PluginJobStatus>>>,
    event_tx: Sender<PluginJobEvent>,
) {
    thread::spawn(move || {
        let result = child.lock().expect("child process poisoned").wait();
        let requested = *kill_requested.lock().expect("job status poisoned");
        let (status, code) = match (requested, result) {
            (Some(status), Ok(exit)) => (status, exit.code()),
            (None, Ok(exit)) if exit.success() => (PluginJobStatus::Exited, exit.code()),
            (None, Ok(exit)) => (PluginJobStatus::Failed, exit.code()),
            (Some(status), Err(_)) => (status, None),
            (None, Err(_)) => (PluginJobStatus::Failed, None),
        };
        event_tx
            .send(PluginJobEvent::Exit {
                job_id,
                status,
                code,
            })
            .ok();
    });
}

fn spawn_timeout_watcher(
    job_id: u64,
    child: Arc<Mutex<Child>>,
    kill_requested: Arc<Mutex<Option<PluginJobStatus>>>,
    timeout_ms: u64,
) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(timeout_ms));
        let mut requested = kill_requested.lock().expect("job status poisoned");
        if requested.is_some() {
            return;
        }
        *requested = Some(PluginJobStatus::TimedOut);
        drop(requested);
        child.lock().expect("child process poisoned").kill().ok();
        tracing::debug!(job_id, timeout_ms, "timed out plugin job");
    });
}

pub(in crate::plugin) fn job_event_to_value(event: &PluginJobEvent) -> Value {
    match event {
        PluginJobEvent::Stdout { job_id, text } | PluginJobEvent::Stderr { job_id, text } => {
            Value::Map(
                HashMap::from([
                    ("job_id".to_string(), Value::Number(*job_id as f64)),
                    (
                        "text".to_string(),
                        Value::String(text.clone().into_boxed_str().into()),
                    ),
                ])
                .into(),
            )
        }
        PluginJobEvent::Exit {
            job_id,
            status,
            code,
        } => Value::Map(
            HashMap::from([
                ("job_id".to_string(), Value::Number(*job_id as f64)),
                ("status".to_string(), Value::String(status.as_str().into())),
                (
                    "code".to_string(),
                    code.map(|code| Value::Number(code as f64))
                        .unwrap_or(Value::Null),
                ),
            ])
            .into(),
        ),
    }
}

pub(in crate::plugin) fn job_id_from_number(value: f64) -> Result<u64, String> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return Err(format!(
            "job id must be a non-negative integer, got {value}"
        ));
    }
    Ok(value as u64)
}
