use super::*;
use crate::editor::pairs;
use crate::editor::{ActionKind, ModeKind};

impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        let insert_mode = action.from_mode == Some(ModeKind::Insert);
        let result = match action.kind.as_ref() {
            Some(ActionKind::MoveLeft) => {
                self.move_cursor_left();
                ActionResult::Handled
            }
            Some(ActionKind::MoveDown) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_down(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveUp) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_up(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveRight) => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Some(ActionKind::InsertChar(c)) => {
                let cursor = self.buffer_view.cursor();
                let auto_close_pairs = insert_mode
                    && globals::with_config(|config| {
                        config.map(|config| config.auto_close_pairs).unwrap_or(true)
                    });

                if auto_close_pairs {
                    if let Some(closer) = pairs::closer_for(*c) {
                        if closer == *c
                            && self
                                .buffer_view
                                .with_buffer(|buffer| buffer.char_at_cursor(cursor) == Some(*c))
                                .unwrap_or(false)
                        {
                            if let Some(new_cursor) = self
                                .buffer_view
                                .with_buffer(|buffer| buffer.next_cursor(cursor))
                                .flatten()
                            {
                                self.buffer_view.set_cursor(new_cursor);
                            }
                        } else {
                            self.insert_pair(*c, closer);
                        }
                    } else if pairs::is_supported_closer(*c)
                        && self
                            .buffer_view
                            .with_buffer(|buffer| buffer.char_at_cursor(cursor) == Some(*c))
                            .unwrap_or(false)
                    {
                        if let Some(new_cursor) = self
                            .buffer_view
                            .with_buffer(|buffer| buffer.next_cursor(cursor))
                            .flatten()
                        {
                            self.buffer_view.set_cursor(new_cursor);
                        }
                    } else {
                        self.insert_char(*c);
                    }
                } else {
                    self.insert_char(*c);
                }
                ActionResult::Handled
            }
            Some(ActionKind::InsertText(text)) => {
                let auto_close_pairs = insert_mode
                    && globals::with_config(|config| {
                        config.map(|config| config.auto_close_pairs).unwrap_or(true)
                    });
                if auto_close_pairs {
                    if let Some((opening, closing)) = pair_text(text) {
                        self.insert_pair(opening, closing);
                    } else {
                        for ch in text.chars() {
                            self.insert_char(ch);
                        }
                    }
                } else {
                    for ch in text.chars() {
                        self.insert_char(ch);
                    }
                }
                ActionResult::Handled
            }
            Some(ActionKind::ForwardTo(boundary)) => {
                self.move_cursor_forward_to(*boundary);
                ActionResult::Handled
            }
            Some(ActionKind::BackTo(boundary)) => {
                self.move_cursor_back_to(*boundary);
                ActionResult::Handled
            }
            Some(ActionKind::MoveToLineEnd) => {
                self.move_cursor_to_line_end();
                ActionResult::Handled
            }
            Some(ActionKind::MoveToLineStart) => {
                self.move_cursor_to_line_start();
                ActionResult::Handled
            }
            Some(ActionKind::MoveToLineContentStart) => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Some(ActionKind::MoveToFirstLine) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(0, target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveToLastLine) => {
                let target_line = self.buffer_view.line_count().saturating_sub(1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(target_line, target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveToScreenTop) => {
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let target_line = start_line.min(self.buffer_view.line_count().saturating_sub(1));
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(target_line, target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveToScreenMiddle) => {
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let line_count = self.buffer_view.line_count();
                if line_count == 0 {
                    return ActionResult::Handled;
                }
                let target_line = (start_line + viewport_rows / 2).min(line_count - 1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(target_line, target_col);
                ActionResult::Handled
            }
            Some(ActionKind::MoveToScreenBottom) => {
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let line_count = self.buffer_view.line_count();
                if line_count == 0 {
                    return ActionResult::Handled;
                }
                let end_line = (start_line + viewport_rows - 1).min(line_count - 1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(end_line, target_col);
                ActionResult::Handled
            }
            Some(ActionKind::DeleteBackward) => {
                if insert_mode {
                    self.delete_insert_char_before_cursor();
                } else {
                    self.delete_char_before_cursor();
                }
                ActionResult::Handled
            }
            Some(ActionKind::DeleteForward) => {
                self.delete_char_at_cursor();
                ActionResult::Handled
            }
            Some(ActionKind::AppendAfterCursor) => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Some(ActionKind::AppendToLineEnd) => {
                let cursor = self.buffer_view.cursor();
                let line_len = self.buffer_view.line_len(cursor.line);
                self.buffer_view
                    .set_cursor(Cursor::new(cursor.line, line_len));
                ActionResult::Handled
            }
            Some(ActionKind::InsertAtLineStart) => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Some(ActionKind::JoinWithSpace) => {
                self.join_lines_with_space();
                ActionResult::Handled
            }
            Some(ActionKind::JoinWithoutSpace) => {
                self.join_lines_without_space();
                ActionResult::Handled
            }
            Some(ActionKind::DeleteLine) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| buffer.delete_lines(cursor.line, 1))
                    .flatten()
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(ActionKind::ChangeLine) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| buffer.change_lines(cursor.line, 1))
                    .flatten()
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(ActionKind::ChangeToLineEnd) => {
                self.handle_count_change_to_line_end(1);
                ActionResult::Handled
            }
            Some(ActionKind::OpenLineBelow) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| buffer.insert_lines_after(cursor.line, 1))
                    .flatten()
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(ActionKind::OpenLineAbove) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| buffer.insert_lines_before(cursor.line, 1))
                    .flatten()
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(ActionKind::ToggleLineComment) => self.handle_count_toggle_line_comment(1),
            Some(ActionKind::MoveToMatchingBracket) => {
                use crate::motion::bracket_matcher::find_matching_bracket;
                let cursor = self.buffer_view.cursor();
                let new_cursor = self
                    .buffer_view
                    .with_buffer(|buffer| find_matching_bracket(buffer, cursor))
                    .flatten();
                if let Some(new_cursor) = new_cursor {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(ActionKind::MoveToPreviousParagraph) => {
                self.move_cursor_to_previous_paragraph();
                ActionResult::Handled
            }
            Some(ActionKind::MoveToNextParagraph) => {
                self.move_cursor_to_next_paragraph();
                ActionResult::Handled
            }
            Some(ActionKind::FindForward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_to_char_forward(*target, 1);
                ActionResult::Handled
            }
            Some(ActionKind::FindBackward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_to_char_backward(*target, 1);
                ActionResult::Handled
            }
            Some(ActionKind::TillForward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_till_forward(*target, 1);
                ActionResult::Handled
            }
            Some(ActionKind::TillBackward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_till_backward(*target, 1);
                ActionResult::Handled
            }
            Some(ActionKind::RepeatLastFind) => {
                if let Some(find) = globals::get_last_find() {
                    match find.direction {
                        globals::Direction::Forward => {
                            if find.kind == globals::FindKind::Find {
                                self.move_cursor_to_char_forward(find.target_char, 1);
                            } else {
                                self.move_cursor_till_forward(find.target_char, 1);
                            }
                        }
                        globals::Direction::Backward => {
                            if find.kind == globals::FindKind::Find {
                                self.move_cursor_to_char_backward(find.target_char, 1);
                            } else {
                                self.move_cursor_till_backward(find.target_char, 1);
                            }
                        }
                    }
                }
                ActionResult::Handled
            }
            Some(ActionKind::RepeatLastFindReverse) => {
                if let Some(find) = globals::get_last_find() {
                    match find.direction {
                        globals::Direction::Forward => {
                            if find.kind == globals::FindKind::Find {
                                self.move_cursor_to_char_backward(find.target_char, 1);
                            } else {
                                self.move_cursor_till_backward(find.target_char, 1);
                            }
                        }
                        globals::Direction::Backward => {
                            if find.kind == globals::FindKind::Find {
                                self.move_cursor_to_char_forward(find.target_char, 1);
                            } else {
                                self.move_cursor_till_forward(find.target_char, 1);
                            }
                        }
                    }
                }
                ActionResult::Handled
            }
            Some(ActionKind::Count(count, inner)) => return self.handle_count(*count, inner),
            Some(ActionKind::Operation(op, target)) => return self.handle_operation(op, target),
            Some(ActionKind::Quit)
            | Some(ActionKind::Undo)
            | Some(ActionKind::Redo)
            | Some(ActionKind::SaveBuffer(_))
            | Some(ActionKind::RepeatLastChange)
            | Some(ActionKind::PreviousTab)
            | Some(ActionKind::NextTab)
            | None => ActionResult::NotHandled,
        };

        if result == ActionResult::Handled && action.resets_remembered_column() {
            self.buffer_view.update_remembered_to_current();
        }

        result
    }
}

fn pair_text(text: &str) -> Option<(char, char)> {
    let mut chars = text.chars();
    let opening = chars.next()?;
    let closing = chars.next()?;
    if chars.next().is_some() {
        return None;
    }

    if pairs::closer_for(opening) == Some(closing) {
        Some((opening, closing))
    } else {
        None
    }
}
