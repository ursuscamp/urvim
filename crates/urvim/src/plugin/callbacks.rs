use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use bearscript::{Engine, Value};
use urvim_core::ui::confirmation_box::PluginConfirmationCancelled;
use urvim_core::ui::input_box::PluginInputCancelled;
use urvim_core::ui::picker::plugin::PluginPickerCancelled;

use super::jobs::PluginJobCallbacks;

pub(in crate::plugin) struct BearscriptPlugin {
    pub(in crate::plugin) engine: Engine,
    pub(in crate::plugin) callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
}

#[derive(Default)]
pub(in crate::plugin) struct BearscriptPluginCallbacks {
    pub(in crate::plugin) apis: HashMap<String, Value>,
    pub(in crate::plugin) api_responses: HashMap<u64, Value>,
    pub(in crate::plugin) commands: HashMap<String, Value>,
    pub(in crate::plugin) event_hooks: HashMap<u64, Value>,
    pub(in crate::plugin) fs: HashMap<u64, Value>,
    pub(in crate::plugin) jobs: HashMap<u64, PluginJobCallbacks>,
    pub(in crate::plugin) syntax_providers: HashMap<u64, Value>,
    pub(in crate::plugin) timers: HashMap<u64, Value>,
    pub(in crate::plugin) pickers: HashMap<u64, PluginPickerCallbacks>,
    pub(in crate::plugin) confirmations: HashMap<u64, PluginConfirmationCallbacks>,
    pub(in crate::plugin) inputs: HashMap<u64, PluginInputCallbacks>,
    pub(in crate::plugin) next_hook_id: u64,
    pub(in crate::plugin) next_syntax_provider_id: u64,
    pub(in crate::plugin) next_picker_id: u64,
    pub(in crate::plugin) next_picker_item_id: u64,
    pub(in crate::plugin) next_confirmation_id: u64,
    pub(in crate::plugin) next_input_id: u64,
    pub(in crate::plugin) picker_cancellation_sender:
        Option<std::sync::mpsc::Sender<PluginPickerCancelled>>,
    pub(in crate::plugin) confirmation_cancellation_sender:
        Option<std::sync::mpsc::Sender<PluginConfirmationCancelled>>,
    pub(in crate::plugin) input_cancellation_sender:
        Option<std::sync::mpsc::Sender<PluginInputCancelled>>,
    pub(in crate::plugin) syntax_refresh_requests: BTreeSet<urvim_core::buffer::BufferId>,
}

pub(in crate::plugin) struct PluginInputCallbacks {
    pub(in crate::plugin) on_submit: Value,
    pub(in crate::plugin) on_cancel: Option<Value>,
}

pub(in crate::plugin) struct PluginConfirmationCallbacks {
    pub(in crate::plugin) on_response: Value,
    pub(in crate::plugin) on_cancel: Option<Value>,
    pub(in crate::plugin) primary_value: Value,
    pub(in crate::plugin) secondary_value: Value,
}

pub(in crate::plugin) struct PluginPickerCallbacks {
    pub(in crate::plugin) on_select: Value,
    pub(in crate::plugin) on_cancel: Option<Value>,
    pub(in crate::plugin) values: HashMap<u64, Value>,
    pub(in crate::plugin) keys: BTreeSet<String>,
}
