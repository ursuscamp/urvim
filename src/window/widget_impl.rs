use super::*;

impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        let result = match action {
            Action::MoveLeft => {
                self.move_cursor_left();
                ActionResult::Handled
            }
            Action::MoveDown => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_down(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveUp => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_up(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveRight => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Action::InsertChar(c) => {
                self.insert_char(*c);
                ActionResult::Handled
            }
            Action::ForwardTo(boundary) => {
                self.move_cursor_forward_to(*boundary);
                ActionResult::Handled
            }
            Action::BackTo(boundary) => {
                self.move_cursor_back_to(*boundary);
                ActionResult::Handled
            }
            Action::MoveToLineEnd => {
                self.move_cursor_to_line_end();
                ActionResult::Handled
            }
            Action::MoveToLineStart => {
                self.move_cursor_to_line_start();
                ActionResult::Handled
            }
            Action::MoveToLineContentStart => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Action::MoveToFirstLine => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(0, target_col);
                ActionResult::Handled
            }
            Action::MoveToLastLine => {
                let target_line = self.buffer_view.line_count().saturating_sub(1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(target_line, target_col);
                ActionResult::Handled
            }
            Action::MoveToScreenTop => {
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
            Action::MoveToScreenMiddle => {
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
            Action::MoveToScreenBottom => {
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
            Action::DeleteBackward => {
                self.delete_char_before_cursor();
                ActionResult::Handled
            }
            Action::DeleteForward => {
                self.delete_char_at_cursor();
                ActionResult::Handled
            }
            Action::AppendAfterCursor => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Action::AppendToLineEnd => {
                let cursor = self.buffer_view.cursor();
                let line_len = self.buffer_view.line_len(cursor.line);
                self.buffer_view
                    .set_cursor(Cursor::new(cursor.line, line_len));
                ActionResult::Handled
            }
            Action::InsertAtLineStart => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Action::JoinWithSpace => {
                self.join_lines_with_space();
                ActionResult::Handled
            }
            Action::JoinWithoutSpace => {
                self.join_lines_without_space();
                ActionResult::Handled
            }
            Action::DeleteLine => {
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
            Action::ChangeLine => {
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
            Action::ChangeToLineEnd => {
                self.handle_count_change_to_line_end(1);
                ActionResult::Handled
            }
            Action::OpenLineBelow => {
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
            Action::OpenLineAbove => {
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
            Action::MoveToMatchingBracket => {
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
            Action::MoveToPreviousParagraph => {
                self.move_cursor_to_previous_paragraph();
                ActionResult::Handled
            }
            Action::MoveToNextParagraph => {
                self.move_cursor_to_next_paragraph();
                ActionResult::Handled
            }
            Action::FindForward(target) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_to_char_forward(*target, 1);
                ActionResult::Handled
            }
            Action::FindBackward(target) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_to_char_backward(*target, 1);
                ActionResult::Handled
            }
            Action::TillForward(target) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_till_forward(*target, 1);
                ActionResult::Handled
            }
            Action::TillBackward(target) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_till_backward(*target, 1);
                ActionResult::Handled
            }
            Action::RepeatLastFind => {
                if let Some(state) = globals::get_last_find() {
                    match (state.kind, state.direction) {
                        (globals::FindKind::Find, globals::Direction::Forward) => {
                            self.move_cursor_to_char_forward(state.target_char, 1)
                        }
                        (globals::FindKind::Find, globals::Direction::Backward) => {
                            self.move_cursor_to_char_backward(state.target_char, 1)
                        }
                        (globals::FindKind::Till, globals::Direction::Forward) => {
                            self.move_cursor_till_forward(state.target_char, 1)
                        }
                        (globals::FindKind::Till, globals::Direction::Backward) => {
                            self.move_cursor_till_backward(state.target_char, 1)
                        }
                    }
                }
                ActionResult::Handled
            }
            Action::RepeatLastFindReverse => {
                if let Some(state) = globals::get_last_find() {
                    match (state.kind, state.direction) {
                        (globals::FindKind::Find, globals::Direction::Forward) => {
                            self.move_cursor_to_char_backward(state.target_char, 1)
                        }
                        (globals::FindKind::Find, globals::Direction::Backward) => {
                            self.move_cursor_to_char_forward(state.target_char, 1)
                        }
                        (globals::FindKind::Till, globals::Direction::Forward) => {
                            self.move_cursor_till_backward(state.target_char, 1)
                        }
                        (globals::FindKind::Till, globals::Direction::Backward) => {
                            self.move_cursor_till_forward(state.target_char, 1)
                        }
                    }
                }
                ActionResult::Handled
            }
            Action::Count(count, inner) => return self.handle_count(*count, inner),
            Action::Operation(op, target) => return self.handle_operation(op, target),
            _ => NotHandled,
        };

        if action.resets_remembered_column() {
            self.buffer_view.update_remembered_to_current();
        }

        result
    }
}
