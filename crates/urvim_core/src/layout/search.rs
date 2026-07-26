use super::Layout;
use crate::buffer::{Cursor, SearchDirection, SearchOptions, TextObjectRange, UndoCheckpoint};
use crate::screen::Screen;
use crate::ui::search_box::{ReplaceChoice, ReplaceConfirmation, SearchBox, SearchBoxOutcome};
use crate::ui::{SearchUiRequest, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;

/// Layout-owned search history and transient UI phase.
#[derive(Debug)]
pub(super) struct SearchState {
    last_query: String,
    last_replacement: String,
    last_options: SearchOptions,
    last_replace_enabled: bool,
    phase: SearchPhase,
}

impl SearchState {
    pub(super) fn new(config: crate::config::SearchConfig) -> Self {
        Self {
            last_query: String::new(),
            last_replacement: String::new(),
            last_options: SearchOptions::new(
                SearchDirection::Forward,
                config.case_sensitive,
                config.regex,
            ),
            last_replace_enabled: config.replace,
            phase: SearchPhase::Idle,
        }
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new(crate::config::SearchConfig::default())
    }
}

#[derive(Debug, Default)]
enum SearchPhase {
    #[default]
    Idle,
    Editing {
        widget: Box<SearchBox>,
        restore: SearchSnapshot,
    },
    Confirming {
        widget: ReplaceConfirmation,
        session: ReplaceSession,
    },
}

#[derive(Debug)]
struct SearchSnapshot {
    query: String,
    options: SearchOptions,
    current_match: Option<Cursor>,
    cursor: Cursor,
}

#[derive(Debug)]
struct ReplaceSession {
    remaining: Vec<PendingReplacement>,
    checkpoint: UndoCheckpoint,
}

#[derive(Debug, Clone)]
struct PendingReplacement {
    range: TextObjectRange,
    text: String,
}

impl Layout {
    pub(super) fn open_search_ui(&mut self, request: SearchUiRequest) {
        self.close_all_dialogs();
        let restore = {
            let view = self.active_buffer_view();
            SearchSnapshot {
                query: view.search_query().to_string(),
                options: view.search_options(),
                current_match: view.current_search_match(),
                cursor: view.cursor(),
            }
        };
        let mut options = self.search.last_options;
        if let Some(direction) = request.direction {
            options.set_direction(direction);
        }
        if let Some(case_sensitive) = request.case_sensitive {
            options.set_case_sensitive(case_sensitive);
        }
        if let Some(regex) = request.regex {
            options.set_regex(regex);
        }
        let replace_enabled = request
            .replace_enabled
            .unwrap_or(self.search.last_replace_enabled);
        let query = request
            .query
            .unwrap_or_else(|| self.search.last_query.clone());
        let replacement = request
            .replacement
            .unwrap_or_else(|| self.search.last_replacement.clone());
        self.search.phase = SearchPhase::Editing {
            widget: Box::new(SearchBox::new(
                query.clone(),
                replacement,
                options,
                replace_enabled,
            )),
            restore,
        };
        self.select_search_query(query, options, true);
    }

    pub(super) fn execute_search(
        &mut self,
        query: String,
        replacement: Option<String>,
        options: SearchOptions,
    ) {
        self.close_all_dialogs();
        self.search.last_query = query.clone();
        self.search.last_options = options;
        self.search.last_replace_enabled = replacement.is_some();
        if let Some(replacement) = replacement {
            self.search.last_replacement = replacement.clone();
            self.select_search_query(query, options, true);
            self.start_replace_confirmation(replacement);
        } else {
            self.select_search_query(query, options, true);
        }
    }

    #[cfg(test)]
    pub(super) fn open_search(&mut self, direction: SearchDirection) {
        self.open_search_ui(SearchUiRequest::with_direction(direction));
    }

    pub(super) fn search_box_is_open(&self) -> bool {
        matches!(self.search.phase, SearchPhase::Editing { .. })
    }

    pub(super) fn replace_confirmation_is_open(&self) -> bool {
        matches!(self.search.phase, SearchPhase::Confirming { .. })
    }

    pub(super) fn search_cursor(&self) -> Option<crate::ui::Position> {
        match &self.search.phase {
            SearchPhase::Editing { widget, .. } => widget.cursor(),
            SearchPhase::Idle | SearchPhase::Confirming { .. } => None,
        }
    }

    pub(super) fn render_search(&mut self, screen: &mut Screen, rect: UiRect) {
        match &mut self.search.phase {
            SearchPhase::Editing { widget, .. } => widget.render_widget(screen, rect, &UiContext),
            SearchPhase::Confirming { widget, .. } => {
                widget.render_widget(screen, rect, &UiContext)
            }
            SearchPhase::Idle => {}
        }
    }

    pub(super) fn handle_search_event(&mut self, event: &UiEvent) -> UiEventResult {
        if let SearchPhase::Confirming { widget, .. } = &mut self.search.phase {
            let result = widget.handle_ui_event(event, &mut UiContext);
            let choice = widget.take_choice();
            if let Some(choice) = choice {
                self.apply_replace_choice(choice);
            }
            return result;
        }

        let (result, live_query, live_options, search_changed, outcome) = {
            let SearchPhase::Editing { widget, .. } = &mut self.search.phase else {
                return UiEventResult::NotHandled;
            };
            let before_query = widget.query().to_string();
            let before_options = widget.options();
            let result = widget.handle_ui_event(event, &mut UiContext);
            let live_query = widget.query().to_string();
            let live_options = widget.options();
            let search_changed = before_query != live_query || before_options != live_options;
            let outcome = widget.take_outcome();
            (result, live_query, live_options, search_changed, outcome)
        };

        if search_changed {
            self.select_search_query(live_query, live_options, true);
        }
        match outcome {
            Some(SearchBoxOutcome::Search {
                query,
                replacement,
                options,
            }) => {
                self.search.last_query = query.clone();
                self.search.last_replacement = replacement;
                self.search.last_options = options;
                self.search.last_replace_enabled = false;
                self.select_search_query(query, options, true);
                self.search.phase = SearchPhase::Idle;
                self.clear_modal_inherited_keys();
            }
            Some(SearchBoxOutcome::Replace {
                query,
                replacement,
                options,
            }) => {
                self.search.last_query = query.clone();
                self.search.last_replacement = replacement.clone();
                self.search.last_options = options;
                self.search.last_replace_enabled = true;
                self.select_search_query(query, options, true);
                self.start_replace_confirmation(replacement);
            }
            Some(SearchBoxOutcome::Cancelled) => {
                let phase = std::mem::take(&mut self.search.phase);
                if let SearchPhase::Editing { restore, .. } = phase {
                    let view = self.active_buffer_view_mut();
                    view.set_search(restore.query, restore.options, restore.current_match);
                    view.set_cursor(restore.cursor);
                }
                self.clear_modal_inherited_keys();
            }
            None => {}
        }
        result
    }

    pub(super) fn select_next_search_match(&mut self) -> bool {
        let direction = self.active_buffer_view().search_options().direction();
        self.select_relative_search_match(direction)
    }

    pub(super) fn select_previous_search_match(&mut self) -> bool {
        let direction = self
            .active_buffer_view()
            .search_options()
            .direction()
            .opposite();
        self.select_relative_search_match(direction)
    }

    fn select_search_query(&mut self, query: String, options: SearchOptions, inclusive: bool) {
        if query.is_empty() {
            self.active_buffer_view_mut().clear_search();
            return;
        }
        let cursor = self.active_buffer_view().cursor();
        let matches = self
            .active_buffer_view()
            .with_buffer(|buffer| buffer.find_search_matches(&query, options))
            .and_then(Result::ok)
            .unwrap_or_default();
        let current = match options.direction() {
            SearchDirection::Forward => matches
                .iter()
                .find(|range| {
                    if inclusive {
                        range.start >= cursor
                    } else {
                        range.start > cursor
                    }
                })
                .or_else(|| matches.first()),
            SearchDirection::Reverse => matches
                .iter()
                .rev()
                .find(|range| {
                    if inclusive {
                        range.start <= cursor
                    } else {
                        range.start < cursor
                    }
                })
                .or_else(|| matches.last()),
        }
        .map(|range| range.start);
        let view = self.active_buffer_view_mut();
        view.set_search(query, options, current);
        if let Some(current) = current {
            view.set_cursor(current);
        }
    }

    fn select_relative_search_match(&mut self, direction: SearchDirection) -> bool {
        let query = self.active_buffer_view().search_query().to_string();
        if query.is_empty() {
            return true;
        }
        let matches = self.active_buffer_view().search_matches();
        if matches.is_empty() {
            let options = self.active_buffer_view().search_options();
            self.active_buffer_view_mut()
                .set_search(query, options, None);
            return true;
        }
        let origin = self
            .active_buffer_view()
            .current_search_match()
            .unwrap_or_else(|| self.active_buffer_view().cursor());
        let selected = match direction {
            SearchDirection::Forward => matches
                .iter()
                .find(|range| range.start > origin)
                .unwrap_or(&matches[0]),
            SearchDirection::Reverse => matches
                .iter()
                .rev()
                .find(|range| range.start < origin)
                .unwrap_or_else(|| matches.last().expect("not empty")),
        };
        let cursor = selected.start;
        let options = self.active_buffer_view().search_options();
        let view = self.active_buffer_view_mut();
        view.set_search(query, options, Some(cursor));
        view.set_cursor(cursor);
        true
    }

    fn start_replace_confirmation(&mut self, replacement: String) {
        let query = self.active_buffer_view().search_query().to_string();
        let options = self.active_buffer_view().search_options();
        let mut replacements = self
            .active_buffer_view()
            .with_buffer(|buffer| buffer.find_search_replacements(&query, &replacement, options))
            .and_then(Result::ok)
            .unwrap_or_default()
            .into_iter()
            .map(|(range, text)| PendingReplacement { range, text })
            .collect::<Vec<_>>();
        if query.is_empty() || replacements.is_empty() {
            self.search.phase = SearchPhase::Idle;
            return;
        }
        let current = self
            .active_buffer_view()
            .current_search_match()
            .unwrap_or(replacements[0].range.start);
        let split = replacements
            .iter()
            .position(|replacement| replacement.range.start == current)
            .unwrap_or(0);
        if options.direction() == SearchDirection::Forward {
            replacements.rotate_left(split);
        } else {
            replacements = (0..replacements.len())
                .map(|offset| {
                    replacements[(split + replacements.len() - offset) % replacements.len()].clone()
                })
                .collect();
        }
        let checkpoint = self
            .active_buffer_view()
            .with_buffer(|buffer| buffer.undo_checkpoint())
            .expect("pooled buffer");
        self.search.phase = SearchPhase::Confirming {
            widget: ReplaceConfirmation::default(),
            session: ReplaceSession {
                remaining: replacements,
                checkpoint,
            },
        };
        self.show_current_replacement();
    }

    fn apply_replace_choice(&mut self, choice: ReplaceChoice) {
        match choice {
            ReplaceChoice::Cancel => self.finish_replacement(),
            ReplaceChoice::Skip => {
                if let SearchPhase::Confirming { session, .. } = &mut self.search.phase
                    && !session.remaining.is_empty()
                {
                    session.remaining.remove(0);
                }
                self.advance_replacement();
            }
            ReplaceChoice::Replace => {
                self.replace_current_match();
                self.advance_replacement();
            }
            ReplaceChoice::All => {
                let edits = match &self.search.phase {
                    SearchPhase::Confirming { session, .. } => session
                        .remaining
                        .iter()
                        .map(|replacement| {
                            (
                                replacement.range.start,
                                replacement.range.end,
                                replacement.text.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    SearchPhase::Idle | SearchPhase::Editing { .. } => Vec::new(),
                };
                self.active_buffer_view().with_buffer_mut(|buffer| {
                    buffer.apply_text_edits(&edits);
                });
                self.finish_replacement();
            }
        }
    }

    fn replace_current_match(&mut self) {
        let Some(replacement) = (match &mut self.search.phase {
            SearchPhase::Confirming { session, .. } if !session.remaining.is_empty() => {
                Some(session.remaining.remove(0))
            }
            SearchPhase::Idle | SearchPhase::Editing { .. } | SearchPhase::Confirming { .. } => {
                None
            }
        }) else {
            return;
        };
        let range = replacement.range;
        self.active_buffer_view().with_buffer_mut(|buffer| {
            buffer.apply_text_edits(&[(range.start, range.end, replacement.text.clone())]);
        });
        let delta = replacement.text.len() as isize - (range.end.col - range.start.col) as isize;
        if let SearchPhase::Confirming { session, .. } = &mut self.search.phase {
            for remaining in &mut session.remaining {
                if remaining.range.start.line == range.start.line
                    && remaining.range.start.col > range.start.col
                {
                    remaining.range.start.col =
                        remaining.range.start.col.saturating_add_signed(delta);
                    remaining.range.end.col = remaining.range.end.col.saturating_add_signed(delta);
                }
            }
        }
    }

    fn advance_replacement(&mut self) {
        let finished = !matches!(
            &self.search.phase,
            SearchPhase::Confirming { session, .. } if !session.remaining.is_empty()
        );
        if finished {
            self.finish_replacement();
        } else {
            self.show_current_replacement();
        }
    }

    fn show_current_replacement(&mut self) {
        let range = match &self.search.phase {
            SearchPhase::Confirming { session, .. } => session
                .remaining
                .first()
                .map(|replacement| replacement.range),
            SearchPhase::Idle | SearchPhase::Editing { .. } => None,
        };
        let Some(range) = range else {
            return;
        };
        let query = self.active_buffer_view().search_query().to_string();
        let options = self.active_buffer_view().search_options();
        let view = self.active_buffer_view_mut();
        view.set_search(query, options, Some(range.start));
        view.set_cursor(range.start);
    }

    pub(super) fn close_search_ui(&mut self) {
        let phase = std::mem::take(&mut self.search.phase);
        if let SearchPhase::Confirming { session, .. } = phase {
            self.active_buffer_view().with_buffer_mut(|buffer| {
                buffer.squash_undo_history(session.checkpoint);
            });
        }
    }

    fn finish_replacement(&mut self) {
        self.close_search_ui();
        let query = self.active_buffer_view().search_query().to_string();
        let options = self.active_buffer_view().search_options();
        self.select_search_query(query, options, true);
        self.clear_modal_inherited_keys();
    }
}
