use std::sync::mpsc::Sender;

use super::Layout;
use crate::buffer::BufferId;
use crate::ui::Command;
use crate::ui::confirmation_box::{
    ConfirmationBox, ConfirmationResponse, PluginConfirmationCancelled, PluginConfirmationId,
    PluginConfirmationSelection,
};
use crate::ui::{Intent, UiEvent, UiEventResult};

pub(super) struct ConfirmationDialog {
    widget: ConfirmationBox,
    plugin: Option<PluginConfirmationOwner>,
}

struct PluginConfirmationOwner {
    plugin: String,
    confirmation_id: PluginConfirmationId,
    cancellation_sender: Sender<PluginConfirmationCancelled>,
}

impl Drop for PluginConfirmationOwner {
    fn drop(&mut self) {
        self.cancellation_sender
            .send(PluginConfirmationCancelled {
                plugin: self.plugin.clone(),
                confirmation_id: self.confirmation_id,
            })
            .ok();
    }
}

impl std::fmt::Debug for ConfirmationDialog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConfirmationDialog")
            .field("widget", &self.widget)
            .field("plugin", &self.plugin.as_ref().map(|owner| &owner.plugin))
            .finish()
    }
}

impl Layout {
    pub(super) fn open_confirmation_box(
        &mut self,
        query: impl Into<String>,
        positive_intent: impl Into<Intent>,
    ) {
        self.clear_modal_inherited_keys();
        self.dialogs.confirmation_box = Some(ConfirmationDialog {
            widget: ConfirmationBox::yes_no(query, positive_intent),
            plugin: None,
        });
    }

    /// Opens a plugin-owned binary confirmation overlay.
    pub fn open_plugin_confirmation(
        &mut self,
        plugin: String,
        confirmation_id: PluginConfirmationId,
        title: String,
        query: String,
        primary_label: String,
        primary_key: char,
        secondary_label: String,
        secondary_key: char,
        cancellation_sender: Sender<PluginConfirmationCancelled>,
    ) {
        self.close_all_dialogs();
        let primary = ConfirmationResponse::new(
            primary_label,
            primary_key,
            Some(Command::PluginConfirmationSelect {
                plugin: plugin.clone(),
                confirmation_id,
                selection: PluginConfirmationSelection::Primary,
            }),
        );
        let secondary = ConfirmationResponse::new(
            secondary_label,
            secondary_key,
            Some(Command::PluginConfirmationSelect {
                plugin: plugin.clone(),
                confirmation_id,
                selection: PluginConfirmationSelection::Secondary,
            }),
        );
        self.dialogs.confirmation_box = Some(ConfirmationDialog {
            widget: ConfirmationBox::new(title, query, primary, secondary),
            plugin: Some(PluginConfirmationOwner {
                plugin,
                confirmation_id,
                cancellation_sender,
            }),
        });
    }

    /// Closes a plugin confirmation owned by `plugin`.
    pub fn close_plugin_confirmation(
        &mut self,
        plugin: &str,
        confirmation_id: PluginConfirmationId,
    ) -> Result<(), String> {
        let dialog = self
            .dialogs
            .confirmation_box
            .as_ref()
            .ok_or_else(|| format!("plugin confirmation {confirmation_id} is not open"))?;
        let owned = dialog.plugin.as_ref().is_some_and(|owner| {
            owner.plugin == plugin && owner.confirmation_id == confirmation_id
        });
        if !owned {
            return Err(format!("plugin confirmation {confirmation_id} is not open"));
        }
        self.close_confirmation_box();
        Ok(())
    }

    /// Closes the open plugin confirmation when it belongs to `plugin`.
    pub fn close_plugin_confirmation_owned(&mut self, plugin: &str) {
        let owned = self
            .dialogs
            .confirmation_box
            .as_ref()
            .and_then(|dialog| dialog.plugin.as_ref())
            .is_some_and(|owner| owner.plugin == plugin);
        if owned {
            self.close_confirmation_box();
        }
    }

    /// Opens a confirmation box for overwriting a file that changed on disk.
    pub fn prompt_overwrite_buffer(&mut self, buffer_id: BufferId) {
        let label = crate::globals::with_buffer(buffer_id, |buffer| {
            buffer
                .path()
                .map(|path| path.as_path().display().to_string())
                .or_else(|| {
                    buffer
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "this buffer".to_string())
        })
        .unwrap_or_else(|| "this buffer".to_string());

        self.open_confirmation_box(
            format!("File {label} changed on disk. Overwrite anyway?"),
            Command::OverwriteBuffer(Some(buffer_id)),
        );
    }

    pub(super) fn close_confirmation_box(&mut self) {
        self.dialogs.confirmation_box = None;
        self.clear_modal_inherited_keys();
    }

    pub(super) fn confirmation_box_is_open(&self) -> bool {
        self.dialogs
            .confirmation_box
            .as_ref()
            .is_some_and(|dialog| dialog.widget.is_open())
    }

    pub(super) fn handle_confirmation_box_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(dialog) = self.dialogs.confirmation_box.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = dialog.widget.handle_ui_event(event, &mut ctx);
        if result.handled() && !dialog.widget.is_open() {
            if matches!(&result, UiEventResult::Handled(intents) if !intents.is_empty()) {
                dialog.plugin = None;
            }
            self.close_confirmation_box();
        }

        result
    }

    pub(super) fn confirmation_box_mut(&mut self) -> Option<&mut ConfirmationBox> {
        self.dialogs
            .confirmation_box
            .as_mut()
            .map(|dialog| &mut dialog.widget)
    }
}
