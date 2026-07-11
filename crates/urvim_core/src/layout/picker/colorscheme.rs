use super::Layout;
use crate::ui::UiEventResult;
use crate::ui::picker::colorscheme::{ColorschemePickerSource, ColorschemePickerWidget};
use crate::widget::Widget;

impl Layout {
    /// Opens the colorscheme picker overlay.
    pub(in crate::layout) fn open_colorscheme_picker(&mut self) {
        self.close_all_dialogs();

        let names: Vec<String> = crate::globals::with_theme_registry(|registry| {
            registry
                .map(|r| r.names().into_iter().map(ToOwned::to_owned).collect())
                .unwrap_or_default()
        });
        let mut picker = ColorschemePickerWidget::new(ColorschemePickerSource::with_jobs(
            names,
            self.jobs.clone(),
        ));
        let mode = picker.source_mut().query_mode();
        picker.set_query_prompt_segments(ColorschemePickerSource::query_prompt_segments(mode));
        picker.set_label("Colorschemes");
        picker.restart_search();
        self.dialogs.colorscheme_picker = Some(picker);
    }

    /// Closes the colorscheme picker overlay.
    pub(in crate::layout) fn close_colorscheme_picker(&mut self) {
        if let Some(picker) = self.dialogs.colorscheme_picker.as_mut() {
            picker.close();
        }
        self.dialogs.colorscheme_picker = None;
        self.clear_modal_inherited_keys();
    }

    /// Returns true when the colorscheme picker is open.
    pub(in crate::layout) fn colorscheme_picker_is_open(&self) -> bool {
        self.dialogs
            .colorscheme_picker
            .as_ref()
            .is_some_and(ColorschemePickerWidget::is_open)
    }

    /// Returns a mutable reference to the colorscheme picker when open.
    pub(in crate::layout) fn colorscheme_picker_mut(
        &mut self,
    ) -> Option<&mut ColorschemePickerWidget> {
        self.dialogs.colorscheme_picker.as_mut()
    }

    /// Routes an event to the colorscheme picker overlay.
    ///
    /// While the picker is open this handles highlight changes by temporarily
    /// applying the highlighted theme. When the picker closes without a
    /// selection (Esc), the theme reverts to the originally configured one.
    pub(in crate::layout) fn handle_colorscheme_picker_event(
        &mut self,
        event: &crate::ui::UiEvent,
    ) -> UiEventResult {
        let previous_highlight = self.dialogs.colorscheme_picker.as_ref().and_then(|p| {
            p.highlighted_index()
                .and_then(|i| p.results().get(i).cloned())
        });

        let result = {
            let Some(picker) = self.dialogs.colorscheme_picker.as_mut() else {
                return UiEventResult::NotHandled;
            };
            let mut ctx = crate::ui::UiContext;
            picker.handle_ui_event(event, &mut ctx)
        };

        let current_highlight = self.dialogs.colorscheme_picker.as_ref().and_then(|p| {
            p.highlighted_index()
                .and_then(|i| p.results().get(i).cloned())
        });
        let is_open = self
            .dialogs
            .colorscheme_picker
            .as_ref()
            .map_or(false, |p| p.is_open());

        if current_highlight.is_some() && current_highlight != previous_highlight {
            if let Some(ref name) = current_highlight {
                self.apply_theme(name);
            }
        }

        if result.handled() && !is_open {
            let intents = result.into_intents();
            if intents.is_empty() {
                if let Some(original) = crate::globals::with_config(|config| config.theme.clone()) {
                    self.apply_theme(&original);
                }
            }
            self.close_colorscheme_picker();
            return UiEventResult::Handled(intents);
        }

        result
    }

    fn apply_theme(&self, name: &str) {
        crate::globals::with_theme_registry(|registry| {
            if let Some(theme) = registry.and_then(|r| r.get(name).cloned()) {
                crate::globals::set_active_theme(theme);
            }
        });
    }
}
