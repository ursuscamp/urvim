use super::Layout;
use crate::config::CompletionTrigger;
use crate::ui::UiEventResult;
use crate::ui::completion::CompletionWidget;
use std::time::Instant;

impl Layout {
    /// Opens the insert-mode completion popup.
    pub(super) fn open_completion(&mut self) {
        self.autocomplete.cancel();
        self.close_all_dialogs();
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let anchor = self.editor_cursor_position();
        let mut completion = CompletionWidget::new(self.jobs.clone());
        completion.set_cursor(anchor);
        completion.request_completion(buffer_id, cursor);
        self.dialogs.completion = Some(completion);
    }

    /// Rebuilds the completion popup from the current buffer state.
    pub fn refresh_completion(&mut self) {
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let anchor = self.editor_cursor_position();
        let Some(completion) = self.dialogs.completion.as_mut() else {
            return;
        };

        completion.set_cursor(anchor);
        completion.request_completion(buffer_id, cursor);
    }

    /// Cancels any pending autocomplete debounce.
    pub fn cancel_autocomplete(&mut self) {
        self.autocomplete.cancel();
    }

    /// Updates the completion/autocomplete state after an insert-mode edit.
    pub fn handle_insert_completion_change(&mut self) {
        let cursor = self.active_buffer_view().cursor();
        let should_autocomplete = self
            .active_buffer_view()
            .with_buffer(|buffer| crate::ui::completion::should_autocomplete(buffer, cursor))
            .unwrap_or(false);

        if self.dialogs.completion.is_some() && !should_autocomplete {
            self.close_completion();
            return;
        }

        if self.completion_is_open() {
            if should_autocomplete {
                self.refresh_completion();
            } else {
                self.close_completion();
            }
            return;
        }

        let trigger =
            crate::globals::with_config(|config| config.completion_trigger).unwrap_or_default();
        if trigger == CompletionTrigger::Auto && should_autocomplete {
            self.autocomplete.schedule(Instant::now());
        } else {
            self.autocomplete.cancel();
        }
    }

    /// Fires autocomplete when the debounce delay has elapsed.
    pub(super) fn maybe_fire_autocomplete(&mut self, now: Instant) -> bool {
        let trigger =
            crate::globals::with_config(|config| config.completion_trigger).unwrap_or_default();
        if trigger != CompletionTrigger::Auto
            || self.completion_is_open()
            || !self.autocomplete.due(now)
        {
            return false;
        }

        let cursor = self.active_buffer_view().cursor();
        let should_trigger = self
            .active_buffer_view()
            .with_buffer(|buffer| crate::ui::completion::should_autocomplete(buffer, cursor))
            .unwrap_or(false);
        if !should_trigger {
            self.autocomplete.cancel();
            return false;
        }

        self.autocomplete.cancel();
        self.open_completion();
        true
    }

    /// Closes the insert-mode completion popup.
    pub(super) fn close_completion(&mut self) {
        if let Some(completion) = self.dialogs.completion.as_mut() {
            completion.close();
        }
        self.dialogs.completion = None;
        self.autocomplete.cancel();
    }

    /// Returns true when the completion popup is open.
    pub(super) fn completion_is_open(&self) -> bool {
        self.dialogs
            .completion
            .as_ref()
            .is_some_and(|completion| completion.is_open())
    }

    /// Routes a UI event to the completion popup.
    pub(super) fn handle_completion_event(&mut self, event: &crate::ui::UiEvent) -> UiEventResult {
        let Some(completion) = self.dialogs.completion.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = completion.handle_ui_event(event, &mut ctx);
        let is_open = completion.is_open();
        let results_empty = completion.results().is_empty();

        if result.handled() && (!is_open || results_empty) {
            let intents = result.into_intents();
            self.close_completion();
            return UiEventResult::Handled(intents);
        }

        if !is_open && !completion.is_pending() {
            self.close_completion();
            return UiEventResult::NotHandled;
        }

        if is_open && results_empty {
            self.close_completion();
            return UiEventResult::Handled(Vec::new());
        }

        result
    }

    /// Routes a completion background job event.
    pub(super) fn handle_completion_job_event(
        &mut self,
        event: &crate::background::JobEvent,
    ) -> bool {
        let active_buffer_id = self.active_buffer_view().buffer_id();
        let Some(completion) = self.dialogs.completion.as_mut() else {
            return false;
        };

        match event {
            crate::background::JobEvent::Completed {
                kind: crate::background::JobKind::Completion(buffer_id, source),
                token,
                payload:
                    Some(crate::background::JobPayload::CompletionResults {
                        source: payload_source,
                        items,
                    }),
            } if *buffer_id == active_buffer_id => {
                if source != payload_source {
                    return false;
                }

                let handled = completion.apply_results(token.generation(), *source, items.clone());
                if handled && !completion.is_open() && !completion.is_pending() {
                    self.close_completion();
                }
                handled
            }
            crate::background::JobEvent::Failed {
                kind: crate::background::JobKind::Completion(buffer_id, source),
                token,
                ..
            } if *buffer_id == active_buffer_id => {
                let handled = completion.apply_results(token.generation(), *source, Vec::new());
                if handled && !completion.is_open() && !completion.is_pending() {
                    self.close_completion();
                }
                handled
            }
            _ => false,
        }
    }
}
