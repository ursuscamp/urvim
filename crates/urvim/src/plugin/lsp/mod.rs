//! Plugin-owned asynchronous LSP request lifecycle.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use bearscript::Value;
use urvim_core::buffer::{BufferId, Cursor};
use urvim_core::globals;
use urvim_core::lsp::runtime::{PendingLspPoll, PendingLspRequest};

/// LSP operation exposed to plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plugin) enum PluginLspRequestKind {
    Hover,
    Definition,
    Completion,
}

impl PluginLspRequestKind {
    pub(in crate::plugin) fn as_str(self) -> &'static str {
        match self {
            Self::Hover => "hover",
            Self::Definition => "definition",
            Self::Completion => "completion",
        }
    }
}

struct PluginLspRequest {
    plugin: String,
    kind: PluginLspRequestKind,
    pending: PendingLspRequest,
    callback: Value,
}

/// One completed request ready for main-thread callback delivery.
pub(in crate::plugin) struct PluginLspOutcome {
    pub(in crate::plugin) id: u64,
    pub(in crate::plugin) plugin: String,
    pub(in crate::plugin) kind: PluginLspRequestKind,
    pub(in crate::plugin) callback: Value,
    pub(in crate::plugin) result: Result<serde_json::Value, String>,
    pub(in crate::plugin) cancelled: bool,
}

/// Main-thread registry for plugin-owned LSP requests.
#[derive(Default)]
pub(in crate::plugin) struct PluginLspRegistry {
    next_id: Cell<u64>,
    requests: RefCell<HashMap<u64, PluginLspRequest>>,
    outcomes: RefCell<VecDeque<PluginLspOutcome>>,
    shutting_down: Cell<bool>,
}

impl PluginLspRegistry {
    pub(in crate::plugin) fn request(
        &self,
        plugin: &str,
        kind: PluginLspRequestKind,
        buffer_id: BufferId,
        cursor: Cursor,
        timeout: Duration,
        callback: Value,
    ) -> Result<u64, String> {
        if self.shutting_down.get() {
            return Err("LSP plugin requests are unavailable during shutdown".to_string());
        }

        let mut pending = globals::try_with_lsp_runtime_mut(|runtime| match kind {
            PluginLspRequestKind::Hover => runtime.request_hover_buffer_async(buffer_id, cursor),
            PluginLspRequestKind::Definition => {
                runtime.request_definition_buffer_async(buffer_id, cursor)
            }
            PluginLspRequestKind::Completion => {
                runtime.request_completion_buffer_async(buffer_id, cursor)
            }
        })
        .ok_or_else(|| "LSP runtime is not available or is busy".to_string())??;
        pending.set_timeout(timeout);

        let id = self.next_id.get().max(1);
        self.next_id.set(id.saturating_add(1));
        self.requests.borrow_mut().insert(
            id,
            PluginLspRequest {
                plugin: plugin.to_string(),
                kind,
                pending,
                callback,
            },
        );
        Ok(id)
    }

    pub(in crate::plugin) fn cancel(&self, plugin: &str, id: u64) -> Result<bool, String> {
        let mut requests = self.requests.borrow_mut();
        let Some(request) = requests.get(&id) else {
            return Ok(false);
        };
        if request.plugin != plugin {
            return Err(format!("LSP request {id} is owned by another plugin"));
        }
        let request = requests.remove(&id).expect("request should still exist");
        let cancellation = request.pending.cancel();
        self.outcomes.borrow_mut().push_back(PluginLspOutcome {
            id,
            plugin: request.plugin,
            kind: request.kind,
            callback: request.callback,
            result: Err("request cancelled".to_string()),
            cancelled: true,
        });
        cancellation.map(|_| true)
    }

    pub(in crate::plugin) fn poll(&self) -> Vec<PluginLspOutcome> {
        let mut outcomes = self.outcomes.borrow_mut().drain(..).collect::<Vec<_>>();
        let requests = self.requests.take();
        for (id, request) in requests {
            let PluginLspRequest {
                plugin,
                kind,
                pending,
                callback,
            } = request;
            match pending.poll() {
                PendingLspPoll::Pending(pending) => {
                    self.requests.borrow_mut().insert(
                        id,
                        PluginLspRequest {
                            plugin,
                            kind,
                            pending,
                            callback,
                        },
                    );
                }
                PendingLspPoll::Ready(result) => outcomes.push(PluginLspOutcome {
                    id,
                    plugin,
                    kind,
                    callback,
                    result,
                    cancelled: false,
                }),
            }
        }
        outcomes
    }

    pub(in crate::plugin) fn remove_plugin(&self, plugin: &str) {
        let ids = self
            .requests
            .borrow()
            .iter()
            .filter_map(|(id, request)| (request.plugin == plugin).then_some(*id))
            .collect::<Vec<_>>();
        let mut requests = self.requests.borrow_mut();
        for id in ids {
            if let Some(request) = requests.remove(&id) {
                request.pending.cancel().ok();
            }
        }
        self.outcomes
            .borrow_mut()
            .retain(|outcome| outcome.plugin != plugin);
    }

    pub(in crate::plugin) fn begin_shutdown(&self) {
        self.shutting_down.set(true);
        let requests = self.requests.take();
        for (id, request) in requests {
            request.pending.cancel().ok();
            self.outcomes.borrow_mut().push_back(PluginLspOutcome {
                id,
                plugin: request.plugin,
                kind: request.kind,
                callback: request.callback,
                result: Err("request cancelled during editor shutdown".to_string()),
                cancelled: true,
            });
        }
    }
}
