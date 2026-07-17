use std::sync::mpsc::Sender;

use super::Layout;
use crate::ui::input_box::{InputBox, InputBoxOutcome, PluginInputCancelled, PluginInputId};
use crate::ui::{Command, Intent, UiEvent, UiEventResult};

pub(super) struct InputDialog {
    pub(super) widget: InputBox,
    plugin: Option<PluginInputOwner>,
}

struct PluginInputOwner {
    plugin: String,
    input_id: PluginInputId,
    cancellation_sender: Sender<PluginInputCancelled>,
    notify_on_drop: bool,
}

impl Drop for PluginInputOwner {
    fn drop(&mut self) {
        if !self.notify_on_drop {
            return;
        }
        self.cancellation_sender
            .send(PluginInputCancelled {
                plugin: self.plugin.clone(),
                input_id: self.input_id,
            })
            .ok();
    }
}

impl std::fmt::Debug for InputDialog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InputDialog")
            .field("widget", &self.widget)
            .field("plugin", &self.plugin.as_ref().map(|owner| &owner.plugin))
            .finish()
    }
}

impl Layout {
    /// Opens a plugin-owned input box overlay.
    pub fn open_plugin_input(
        &mut self,
        plugin: String,
        input_id: PluginInputId,
        title: String,
        prompt: String,
        initial_text: String,
        cancellation_sender: Sender<PluginInputCancelled>,
    ) {
        self.close_all_dialogs();
        self.dialogs.input_box = Some(InputDialog {
            widget: InputBox::new(title, prompt, initial_text),
            plugin: Some(PluginInputOwner {
                plugin,
                input_id,
                cancellation_sender,
                notify_on_drop: true,
            }),
        });
    }

    /// Closes a plugin input box owned by `plugin`.
    pub fn close_plugin_input(
        &mut self,
        plugin: &str,
        input_id: PluginInputId,
    ) -> Result<(), String> {
        let dialog = self
            .dialogs
            .input_box
            .as_ref()
            .ok_or_else(|| format!("plugin input {input_id} is not open"))?;
        let owned = dialog
            .plugin
            .as_ref()
            .is_some_and(|owner| owner.plugin == plugin && owner.input_id == input_id);
        if !owned {
            return Err(format!("plugin input {input_id} is not open"));
        }
        self.close_input_box();
        Ok(())
    }

    /// Closes the open plugin input box when it belongs to `plugin`.
    pub fn close_plugin_input_owned(&mut self, plugin: &str) {
        let owned = self
            .dialogs
            .input_box
            .as_ref()
            .and_then(|dialog| dialog.plugin.as_ref())
            .is_some_and(|owner| owner.plugin == plugin);
        if owned {
            self.close_input_box();
        }
    }

    pub(super) fn close_input_box(&mut self) {
        self.dialogs.input_box = None;
        self.clear_modal_inherited_keys();
    }

    pub(super) fn input_box_is_open(&self) -> bool {
        self.dialogs
            .input_box
            .as_ref()
            .is_some_and(|dialog| dialog.widget.is_open())
    }

    pub(super) fn handle_input_box_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(dialog) = self.dialogs.input_box.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = dialog.widget.handle_ui_event(event, &mut ctx);
        let outcome = dialog.widget.take_outcome();
        let result = match outcome {
            Some(InputBoxOutcome::Submitted(text)) => {
                if let Some(owner) = dialog.plugin.as_mut() {
                    owner.notify_on_drop = false;
                    UiEventResult::Handled(vec![Intent::Command(Command::PluginInputSubmit {
                        plugin: owner.plugin.clone(),
                        input_id: owner.input_id,
                        text,
                    })])
                } else {
                    result
                }
            }
            Some(InputBoxOutcome::Cancelled) | None => result,
        };
        if !dialog.widget.is_open() {
            self.close_input_box();
        }
        result
    }

    pub(super) fn input_box_mut(&mut self) -> Option<&mut InputBox> {
        self.dialogs
            .input_box
            .as_mut()
            .map(|dialog| &mut dialog.widget)
    }
}
