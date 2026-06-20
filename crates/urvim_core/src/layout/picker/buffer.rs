use super::Layout;
use crate::ui::UiContext;
use crate::ui::UiEvent;
use crate::ui::UiEventResult;
use crate::ui::picker::buffer::{BufferPickerSource, BufferPickerWidget};
use crate::widget::Widget;

impl Layout {
    /// Opens the visible-buffer picker overlay.
    pub(in crate::layout) fn open_buffer_picker(&mut self) {
        self.close_all_dialogs();

        let items = self.visible_buffer_items();
        let mut picker =
            BufferPickerWidget::new(BufferPickerSource::with_jobs(items, self.jobs.clone()));
        let mode = picker.source_mut().query_mode();
        picker.set_query_prompt_segments(BufferPickerSource::query_prompt_segments(mode));
        picker.set_label("Buffers");
        picker.restart_search();
        self.dialogs.buffer_picker = Some(picker);
    }

    /// Closes the visible-buffer picker overlay.
    pub(in crate::layout) fn close_buffer_picker(&mut self) {
        if let Some(picker) = self.dialogs.buffer_picker.as_mut() {
            picker.close();
        }
        self.dialogs.buffer_picker = None;
    }

    /// Returns true when the visible-buffer picker is open.
    pub(in crate::layout) fn buffer_picker_is_open(&self) -> bool {
        self.dialogs
            .buffer_picker
            .as_ref()
            .is_some_and(BufferPickerWidget::is_open)
    }

    /// Returns a mutable reference to the visible-buffer picker when open.
    pub(in crate::layout) fn buffer_picker_mut(&mut self) -> Option<&mut BufferPickerWidget> {
        self.dialogs.buffer_picker.as_mut()
    }

    /// Routes an event to the visible-buffer picker overlay.
    pub(in crate::layout) fn handle_buffer_picker_event(
        &mut self,
        event: &UiEvent,
    ) -> UiEventResult {
        let result = {
            let Some(picker) = self.dialogs.buffer_picker.as_mut() else {
                return UiEventResult::NotHandled;
            };

            let mut ctx = UiContext;
            picker.handle_ui_event(event, &mut ctx)
        };

        if result.handled() && !self.buffer_picker_is_open() {
            self.close_buffer_picker();
        }

        result
    }
}
