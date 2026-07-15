use std::sync::mpsc::Sender;

use super::Layout;
use crate::ui::UiEventResult;
use crate::ui::picker::plugin::{
    PluginPickerCancelled, PluginPickerId, PluginPickerItem, PluginPickerSource, PluginPickerWidget,
};
use crate::widget::Widget;

impl Layout {
    /// Opens a plugin-owned picker overlay.
    pub fn open_plugin_picker(
        &mut self,
        plugin: String,
        picker_id: PluginPickerId,
        title: String,
        items: Vec<PluginPickerItem>,
        cancellation_sender: Sender<PluginPickerCancelled>,
    ) {
        self.close_all_dialogs();

        let source = PluginPickerSource::new(
            plugin,
            picker_id,
            items,
            cancellation_sender,
            self.jobs.clone(),
        );
        let mut picker = PluginPickerWidget::new(source);
        let mode = picker.source_mut().query_mode();
        picker.set_query_prompt_segments(PluginPickerSource::query_prompt_segments(mode));
        picker.set_label(title);
        picker.restart_search();
        self.dialogs.plugin_picker = Some(picker);
    }

    /// Replaces items in an open plugin picker owned by `plugin`.
    pub fn set_plugin_picker_items(
        &mut self,
        plugin: &str,
        picker_id: PluginPickerId,
        items: Vec<PluginPickerItem>,
    ) -> Result<(), String> {
        let picker = self.owned_plugin_picker_mut(plugin, picker_id)?;
        picker.source_mut().set_items(items);
        picker.restart_search();
        Ok(())
    }

    /// Appends items to an open plugin picker owned by `plugin`.
    pub fn append_plugin_picker_items(
        &mut self,
        plugin: &str,
        picker_id: PluginPickerId,
        items: Vec<PluginPickerItem>,
    ) -> Result<(), String> {
        let picker = self.owned_plugin_picker_mut(plugin, picker_id)?;
        picker.source_mut().append_items(items);
        picker.restart_search();
        Ok(())
    }

    /// Closes an open plugin picker owned by `plugin`.
    pub fn close_plugin_picker(
        &mut self,
        plugin: &str,
        picker_id: PluginPickerId,
    ) -> Result<(), String> {
        self.owned_plugin_picker_mut(plugin, picker_id)?.close();
        self.dialogs.plugin_picker = None;
        self.clear_modal_inherited_keys();
        Ok(())
    }

    /// Closes the open plugin picker when it belongs to `plugin`.
    pub fn close_plugin_picker_owned(&mut self, plugin: &str) {
        let owned = self
            .dialogs
            .plugin_picker
            .as_mut()
            .is_some_and(|picker| picker.source_mut().plugin() == plugin);
        if owned {
            self.dialogs.plugin_picker = None;
            self.clear_modal_inherited_keys();
        }
    }

    /// Returns true when a plugin picker is open.
    pub fn plugin_picker_is_open(&self) -> bool {
        self.dialogs
            .plugin_picker
            .as_ref()
            .is_some_and(PluginPickerWidget::is_open)
    }

    /// Returns the open plugin picker.
    pub fn plugin_picker_mut(&mut self) -> Option<&mut PluginPickerWidget> {
        self.dialogs.plugin_picker.as_mut()
    }

    /// Routes an event to the plugin picker overlay.
    pub fn handle_plugin_picker_event(&mut self, event: &crate::ui::UiEvent) -> UiEventResult {
        let result = {
            let Some(picker) = self.dialogs.plugin_picker.as_mut() else {
                return UiEventResult::NotHandled;
            };
            let mut ctx = crate::ui::UiContext;
            picker.handle_ui_event(event, &mut ctx)
        };

        if result.handled() && !self.plugin_picker_is_open() {
            self.dialogs.plugin_picker = None;
            self.clear_modal_inherited_keys();
        }

        result
    }

    fn owned_plugin_picker_mut(
        &mut self,
        plugin: &str,
        picker_id: PluginPickerId,
    ) -> Result<&mut PluginPickerWidget, String> {
        let picker = self
            .dialogs
            .plugin_picker
            .as_mut()
            .ok_or_else(|| format!("plugin picker {picker_id} is not open"))?;
        let source = picker.source_mut();
        if source.picker_id() != picker_id || source.plugin() != plugin {
            return Err(format!(
                "plugin picker {picker_id} is not owned by {plugin:?}"
            ));
        }
        Ok(picker)
    }
}
