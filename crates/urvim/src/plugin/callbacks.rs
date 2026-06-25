use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use bearscript::{Engine, Value};

use super::jobs::PluginJobCallbacks;

pub(in crate::plugin) struct BearscriptPlugin {
    pub(in crate::plugin) engine: Engine,
    pub(in crate::plugin) callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
}

#[derive(Default)]
pub(in crate::plugin) struct BearscriptPluginCallbacks {
    pub(in crate::plugin) commands: HashMap<String, Value>,
    pub(in crate::plugin) event_hooks: HashMap<u64, Value>,
    pub(in crate::plugin) fs: HashMap<u64, Value>,
    pub(in crate::plugin) jobs: HashMap<u64, PluginJobCallbacks>,
    pub(in crate::plugin) syntax_providers: HashMap<u64, Value>,
    pub(in crate::plugin) timers: HashMap<u64, Value>,
    pub(in crate::plugin) next_hook_id: u64,
    pub(in crate::plugin) next_syntax_provider_id: u64,
    pub(in crate::plugin) syntax_refresh_requests: BTreeSet<urvim_core::buffer::BufferId>,
}
