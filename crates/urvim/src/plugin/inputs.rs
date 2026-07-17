use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use urvim_core::ui::input_box::PluginInputCancelled;

/// Main-thread event channel for plugin input box lifecycle notifications.
pub(in crate::plugin) struct PluginInputEvents {
    sender: Sender<PluginInputCancelled>,
    receiver: Receiver<PluginInputCancelled>,
}

impl Default for PluginInputEvents {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self { sender, receiver }
    }
}

impl PluginInputEvents {
    /// Returns a sender for input boxes to report cancellation.
    pub(in crate::plugin) fn sender(&self) -> Sender<PluginInputCancelled> {
        self.sender.clone()
    }

    /// Polls the next queued input cancellation.
    pub(in crate::plugin) fn poll(&self) -> Option<PluginInputCancelled> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}
