use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use urvim_core::ui::confirmation_box::PluginConfirmationCancelled;

/// Main-thread event channel for plugin confirmation lifecycle notifications.
pub(in crate::plugin) struct PluginConfirmationEvents {
    sender: Sender<PluginConfirmationCancelled>,
    receiver: Receiver<PluginConfirmationCancelled>,
}

impl Default for PluginConfirmationEvents {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self { sender, receiver }
    }
}

impl PluginConfirmationEvents {
    /// Returns a sender for confirmation dialogs to report cancellation.
    pub(in crate::plugin) fn sender(&self) -> Sender<PluginConfirmationCancelled> {
        self.sender.clone()
    }

    /// Polls the next queued confirmation cancellation.
    pub(in crate::plugin) fn poll(&self) -> Option<PluginConfirmationCancelled> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}
