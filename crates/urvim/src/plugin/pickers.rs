use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use urvim_core::ui::picker::plugin::PluginPickerCancelled;

/// Main-thread event channel for plugin picker lifecycle notifications.
pub(in crate::plugin) struct PluginPickerEvents {
    sender: Sender<PluginPickerCancelled>,
    receiver: Receiver<PluginPickerCancelled>,
}

impl Default for PluginPickerEvents {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self { sender, receiver }
    }
}

impl PluginPickerEvents {
    /// Returns a sender for picker sources to report cancellation.
    pub(in crate::plugin) fn sender(&self) -> Sender<PluginPickerCancelled> {
        self.sender.clone()
    }

    /// Polls the next queued picker cancellation.
    pub(in crate::plugin) fn poll(&self) -> Option<PluginPickerCancelled> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}
