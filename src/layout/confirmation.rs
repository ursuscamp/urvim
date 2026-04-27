use super::Layout;
use crate::ui::confirmation_box::ConfirmationBox;
use crate::ui::{Intent, UiEvent, UiEventResult};

impl Layout {
    pub(super) fn open_confirmation_box(
        &mut self,
        query: impl Into<String>,
        positive_intent: impl Into<Intent>,
    ) {
        self.command_line_open = false;
        self.close_file_picker();
        self.confirmation_box = Some(ConfirmationBox::new(query, positive_intent));
    }

    pub(super) fn close_confirmation_box(&mut self) {
        self.confirmation_box = None;
    }

    pub(super) fn confirmation_box_is_open(&self) -> bool {
        self.confirmation_box
            .as_ref()
            .is_some_and(ConfirmationBox::is_open)
    }

    pub(super) fn handle_confirmation_box_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(prompt) = self.confirmation_box.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = prompt.handle_ui_event(event, &mut ctx);
        if result.handled() && !prompt.is_open() {
            self.close_confirmation_box();
        }

        result
    }

    pub(super) fn confirmation_box_mut(&mut self) -> Option<&mut ConfirmationBox> {
        self.confirmation_box.as_mut()
    }
}
