use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use bearscript::Value;

#[derive(Clone, Debug)]
pub(in crate::plugin) enum PluginFsEvent {
    ReadFile {
        request_id: u64,
        path: String,
        result: Result<String, String>,
    },
    WriteFile {
        request_id: u64,
        path: String,
        result: Result<(), String>,
    },
    ReadDir {
        request_id: u64,
        path: String,
        result: Result<Vec<PluginFsDirEntry>, String>,
    },
}

#[derive(Clone, Debug)]
pub(in crate::plugin) struct PluginFsDirEntry {
    path: String,
    name: String,
    kind: PluginFsDirEntryKind,
}

#[derive(Clone, Copy, Debug)]
enum PluginFsDirEntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl PluginFsDirEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dir => "dir",
            Self::Symlink => "symlink",
            Self::Other => "other",
        }
    }
}

pub(in crate::plugin) struct PluginFsRequest {
    pub(in crate::plugin) id: u64,
}

pub(in crate::plugin) struct PluginFsRegistry {
    next_id: AtomicU64,
    requests: Mutex<HashMap<u64, String>>,
    event_tx: Sender<PluginFsEvent>,
    event_rx: Mutex<Receiver<PluginFsEvent>>,
}

impl Default for PluginFsRegistry {
    fn default() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            next_id: AtomicU64::new(1),
            requests: Mutex::new(HashMap::new()),
            event_tx,
            event_rx: Mutex::new(event_rx),
        }
    }
}

impl PluginFsRegistry {
    pub(in crate::plugin) fn read_file(&self, plugin: &str, path: String) -> PluginFsRequest {
        let request = self.insert_request(plugin);
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = std::fs::read_to_string(&path).map_err(|error| error.to_string());
            event_tx
                .send(PluginFsEvent::ReadFile {
                    request_id: request.id,
                    path,
                    result,
                })
                .ok();
        });
        request
    }

    pub(in crate::plugin) fn write_file(
        &self,
        plugin: &str,
        path: String,
        text: String,
    ) -> PluginFsRequest {
        let request = self.insert_request(plugin);
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = std::fs::write(&path, text).map_err(|error| error.to_string());
            event_tx
                .send(PluginFsEvent::WriteFile {
                    request_id: request.id,
                    path,
                    result,
                })
                .ok();
        });
        request
    }

    pub(in crate::plugin) fn read_dir(&self, plugin: &str, path: String) -> PluginFsRequest {
        let request = self.insert_request(plugin);
        let event_tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = read_dir_entries(&path);
            event_tx
                .send(PluginFsEvent::ReadDir {
                    request_id: request.id,
                    path,
                    result,
                })
                .ok();
        });
        request
    }

    pub(in crate::plugin) fn poll_event(&self) -> Option<PluginFsEvent> {
        self.event_rx
            .lock()
            .expect("filesystem event queue poisoned")
            .try_recv()
            .ok()
    }

    pub(in crate::plugin) fn mark_finished(&self, request_id: u64) -> Option<String> {
        self.requests
            .lock()
            .expect("filesystem registry poisoned")
            .remove(&request_id)
    }

    fn insert_request(&self, plugin: &str) -> PluginFsRequest {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.requests
            .lock()
            .expect("filesystem registry poisoned")
            .insert(id, plugin.to_string());
        PluginFsRequest { id }
    }
}

fn read_dir_entries(path: &str) -> Result<Vec<PluginFsDirEntry>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let kind = if file_type.is_file() {
            PluginFsDirEntryKind::File
        } else if file_type.is_dir() {
            PluginFsDirEntryKind::Dir
        } else if file_type.is_symlink() {
            PluginFsDirEntryKind::Symlink
        } else {
            PluginFsDirEntryKind::Other
        };
        entries.push(PluginFsDirEntry {
            path: entry.path().to_string_lossy().to_string(),
            name: entry.file_name().to_string_lossy().to_string(),
            kind,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub(in crate::plugin) fn fs_event_id(event: &PluginFsEvent) -> u64 {
    match event {
        PluginFsEvent::ReadFile { request_id, .. }
        | PluginFsEvent::WriteFile { request_id, .. }
        | PluginFsEvent::ReadDir { request_id, .. } => *request_id,
    }
}

pub(in crate::plugin) fn fs_event_to_value(event: &PluginFsEvent) -> Value {
    match event {
        PluginFsEvent::ReadFile {
            request_id,
            path,
            result,
        } => match result {
            Ok(text) => success_payload(
                *request_id,
                path,
                [("text", Value::String(text.clone().into_boxed_str().into()))],
            ),
            Err(error) => error_payload(*request_id, path, error),
        },
        PluginFsEvent::WriteFile {
            request_id,
            path,
            result,
        } => match result {
            Ok(()) => success_payload(*request_id, path, []),
            Err(error) => error_payload(*request_id, path, error),
        },
        PluginFsEvent::ReadDir {
            request_id,
            path,
            result,
        } => match result {
            Ok(entries) => success_payload(
                *request_id,
                path,
                [(
                    "entries",
                    Value::List(
                        entries
                            .iter()
                            .map(dir_entry_to_value)
                            .collect::<Vec<_>>()
                            .into(),
                    ),
                )],
            ),
            Err(error) => error_payload(*request_id, path, error),
        },
    }
}

fn success_payload<const N: usize>(
    request_id: u64,
    path: &str,
    fields: [(&str, Value); N],
) -> Value {
    let mut payload = base_payload(request_id, path, true);
    for (key, value) in fields {
        payload.insert(key.to_string(), value);
    }
    Value::Map(payload.into())
}

fn error_payload(request_id: u64, path: &str, error: &str) -> Value {
    let mut payload = base_payload(request_id, path, false);
    payload.insert(
        "error".to_string(),
        Value::String(error.to_string().into_boxed_str().into()),
    );
    Value::Map(payload.into())
}

fn base_payload(request_id: u64, path: &str, ok: bool) -> HashMap<String, Value> {
    HashMap::from([
        ("id".to_string(), Value::Number(request_id as f64)),
        (
            "path".to_string(),
            Value::String(path.to_string().into_boxed_str().into()),
        ),
        ("ok".to_string(), Value::Bool(ok)),
    ])
}

fn dir_entry_to_value(entry: &PluginFsDirEntry) -> Value {
    Value::Map(
        HashMap::from([
            (
                "path".to_string(),
                Value::String(entry.path.clone().into_boxed_str().into()),
            ),
            (
                "name".to_string(),
                Value::String(entry.name.clone().into_boxed_str().into()),
            ),
            (
                "kind".to_string(),
                Value::String(entry.kind.as_str().into()),
            ),
        ])
        .into(),
    )
}
