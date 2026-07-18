use super::*;
use crate::editor::pairs;
use crate::editor::{EditorOperation, ModeKind};
use crate::register::{DefaultRegisterRole, RegisterContentKind};

impl EditorTab {
    /// Dispatches an editor action to this tab.
    pub fn dispatch_action(&mut self, action: &EditorAction) -> ActionResult {
        self.pending_repeat_suffix = None;
        let insert_mode = action.from_mode == Some(ModeKind::Insert);
        let result = match action.kind.as_ref() {
            Some(EditorOperation::MoveLeft) => {
                if action.from_mode == Some(ModeKind::Insert) {
                    self.move_cursor_left();
                } else {
                    self.move_cursor_left_within_line();
                }
                ActionResult::Handled
            }
            Some(EditorOperation::MoveDown) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_down(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveUp) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_up(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Some(EditorOperation::MovePageUp) => {
                self.move_cursor_page_up(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::MovePageDown) => {
                self.move_cursor_page_down(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveHalfPageUp) => {
                self.move_cursor_half_page_up(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveHalfPageDown) => {
                self.move_cursor_half_page_down(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveRight) => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Some(EditorOperation::InsertChar(c)) => {
                let cursor = self.buffer_view.cursor();
                let auto_close_pairs = insert_mode
                    && globals::with_config(|config| config.auto_close_pairs).unwrap_or(true);

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
            Some(EditorOperation::InsertText(text)) => {
                let auto_close_pairs = insert_mode
                    && globals::with_config(|config| config.auto_close_pairs).unwrap_or(true);
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
            Some(EditorOperation::InsertRawPaste(text)) => {
                if self.insert_raw_text(text).is_some() {
                    ActionResult::Handled
                } else {
                    ActionResult::NotHandled
                }
            }
            Some(EditorOperation::ReplaceSelectionRawPaste(text)) => {
                if self
                    .replace_visual_selection_with_raw_text(text, action.from_mode)
                    .is_some()
                {
                    ActionResult::Handled
                } else {
                    ActionResult::NotHandled
                }
            }
            Some(EditorOperation::InsertNewline) => {
                self.pending_repeat_suffix = if action.from_mode == Some(ModeKind::Replace) {
                    self.replace_newline_at_cursor()
                } else {
                    self.insert_newline()
                };
                ActionResult::Handled
            }
            Some(EditorOperation::ForwardTo(boundary)) => {
                self.move_cursor_forward_to(*boundary);
                ActionResult::Handled
            }
            Some(EditorOperation::BackTo(boundary)) => {
                self.move_cursor_back_to(*boundary);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToLineEnd) => {
                self.move_cursor_to_line_end();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToLineStart) => {
                self.move_cursor_to_line_start();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToLineContentStart) => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToFirstLine) => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(0, target_col);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToLastLine) => {
                let target_line = self.buffer_view.line_count().saturating_sub(1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.set_cursor_to_visual_col_on_line(target_line, target_col);
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToScreenTop) => {
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
            Some(EditorOperation::MoveToScreenMiddle) => {
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
            Some(EditorOperation::MoveToScreenBottom) => {
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
            Some(EditorOperation::ViewportCursorTop) => {
                self.align_viewport_cursor_top(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::ViewportCursorCenter) => {
                self.align_viewport_cursor_center(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::ViewportCursorBottom) => {
                self.align_viewport_cursor_bottom(self.size.rows as usize);
                ActionResult::Handled
            }
            Some(EditorOperation::ToggleFold) => {
                self.buffer_view.toggle_fold_at_cursor();
                ActionResult::Handled
            }
            Some(EditorOperation::OpenFold) => {
                self.buffer_view.open_fold_at_cursor();
                ActionResult::Handled
            }
            Some(EditorOperation::CloseFold) => {
                self.buffer_view.close_fold_at_cursor();
                ActionResult::Handled
            }
            Some(EditorOperation::DeleteBackward) => {
                if insert_mode {
                    if !self.delete_insert_indent_before_cursor() {
                        self.delete_insert_char_before_cursor();
                    }
                } else {
                    let cursor = self.buffer_view.cursor();
                    let text = self
                        .buffer_view
                        .with_buffer(|buffer| {
                            let start = buffer.prev_cursor(cursor)?;
                            let end = cursor;
                            Some((start, end))
                        })
                        .flatten()
                        .and_then(|(start, end)| self.capture_characterwise_text(start, end));
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Delete,
                        text,
                        RegisterContentKind::Characterwise,
                    );
                    self.delete_char_before_cursor();
                }
                ActionResult::Handled
            }
            Some(EditorOperation::IndentDecrease) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self.shift_lines_indentation(
                    cursor.line,
                    1,
                    crate::buffer::IndentDirection::Decrease,
                ) {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(EditorOperation::IndentIncrease) => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self.shift_lines_indentation(
                    cursor.line,
                    1,
                    crate::buffer::IndentDirection::Increase,
                ) {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(EditorOperation::DeleteForward) => {
                let cursor = self.buffer_view.cursor();
                let text = self
                    .buffer_view
                    .with_buffer(|buffer| buffer.next_cursor(cursor))
                    .flatten()
                    .and_then(|end| self.capture_characterwise_text(cursor, end));
                self.store_register_text(
                    action.register,
                    DefaultRegisterRole::Delete,
                    text,
                    RegisterContentKind::Characterwise,
                );
                self.delete_char_at_cursor();
                ActionResult::Handled
            }
            Some(EditorOperation::DeleteSelection) => {
                if action.from_mode == Some(ModeKind::VisualLine) {
                    let content = self.buffer_view.visual_line_selection_range().and_then(
                        |(start_line, count)| self.capture_linewise_text(start_line, count),
                    );
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Delete,
                        content,
                        RegisterContentKind::Linewise,
                    );
                    self.delete_linewise_visual_selection();
                } else {
                    let content = self
                        .buffer_view
                        .visual_selection_range()
                        .and_then(|range| self.capture_characterwise_text(range.start, range.end));
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Delete,
                        content,
                        RegisterContentKind::Characterwise,
                    );
                    self.delete_visual_selection();
                }
                ActionResult::Handled
            }
            Some(EditorOperation::YankSelection) => {
                if action.from_mode == Some(ModeKind::VisualLine) {
                    let content = self.buffer_view.visual_line_selection_range().and_then(
                        |(start_line, count)| self.capture_linewise_text(start_line, count),
                    );
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Yank,
                        content,
                        RegisterContentKind::Linewise,
                    );
                } else {
                    let content = self
                        .buffer_view
                        .visual_selection_range()
                        .and_then(|range| self.capture_characterwise_text(range.start, range.end));
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Yank,
                        content,
                        RegisterContentKind::Characterwise,
                    );
                }
                ActionResult::Handled
            }
            Some(EditorOperation::YankLine) => {
                let cursor = self.buffer_view.cursor();
                self.store_register_text(
                    action.register,
                    DefaultRegisterRole::Yank,
                    self.capture_linewise_text(cursor.line, 1),
                    RegisterContentKind::Linewise,
                );
                self.buffer_view.begin_yank_flash(
                    YankFlashSelection::Line {
                        start_line: cursor.line,
                        count: 1,
                    },
                    std::time::Duration::from_millis(200),
                );
                ActionResult::Handled
            }
            Some(EditorOperation::AppendAfterCursor) => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Some(EditorOperation::AppendToLineEnd) => {
                let cursor = self.buffer_view.cursor();
                let line_len = self.buffer_view.line_len(cursor.line);
                self.buffer_view
                    .set_cursor(Cursor::new(cursor.line, line_len));
                ActionResult::Handled
            }
            Some(EditorOperation::InsertAtLineStart) => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Some(EditorOperation::JoinWithSpace) => {
                self.join_lines_with_space();
                ActionResult::Handled
            }
            Some(EditorOperation::JoinWithoutSpace) => {
                self.join_lines_without_space();
                ActionResult::Handled
            }
            Some(EditorOperation::DeleteLine) => {
                let cursor = self.buffer_view.cursor();
                self.store_register_text(
                    action.register,
                    DefaultRegisterRole::Delete,
                    self.capture_linewise_text(cursor.line, 1),
                    RegisterContentKind::Linewise,
                );
                if let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| buffer.delete_lines(cursor.line, 1))
                    .flatten()
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(EditorOperation::ChangeLine) => {
                let cursor = self.buffer_view.cursor();
                self.store_register_text(
                    action.register,
                    DefaultRegisterRole::Change,
                    self.capture_linewise_text(cursor.line, 1),
                    RegisterContentKind::Linewise,
                );
                self.change_lines_with_auto_indent(1)
            }
            Some(EditorOperation::ChangeSelection) => {
                if action.from_mode == Some(ModeKind::VisualLine) {
                    let content = self.buffer_view.visual_line_selection_range().and_then(
                        |(start_line, count)| self.capture_linewise_text(start_line, count),
                    );
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Change,
                        content,
                        RegisterContentKind::Linewise,
                    );
                    self.change_linewise_visual_selection();
                } else {
                    let content = self
                        .buffer_view
                        .visual_selection_range()
                        .and_then(|range| self.capture_characterwise_text(range.start, range.end));
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Change,
                        content,
                        RegisterContentKind::Characterwise,
                    );
                    self.change_visual_selection();
                }
                ActionResult::Handled
            }
            Some(EditorOperation::ChangeToLineEnd) => {
                self.handle_count_change_to_line_end(1, action.register);
                ActionResult::Handled
            }
            Some(EditorOperation::PasteAfter) => self.paste_register_content(action.register, true),
            Some(EditorOperation::PasteBefore) => {
                self.paste_register_content(action.register, false)
            }
            Some(EditorOperation::OpenLineBelow) => {
                let cursor = self.buffer_view.cursor();
                let prefix = self.inferred_newline_prefix(cursor);
                let insert_after_line = self
                    .buffer_view
                    .folded_range_at_visible_start(cursor.line)
                    .map(|range| range.start_line)
                    .unwrap_or(cursor.line);
                self.buffer_view.open_fold_starting_at(insert_after_line);
                if let Some(new_cursor) =
                    self.insert_auto_indented_lines_after(insert_after_line, 1, prefix)
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(EditorOperation::OpenLineAbove) => {
                let cursor = self.buffer_view.cursor();
                let prefix = self.inferred_newline_prefix(cursor);
                let folded_boundary = self
                    .buffer_view
                    .folded_range_before_visible_line(cursor.line);
                let insert_before_line = cursor.line;
                if let Some(range) = folded_boundary {
                    self.buffer_view.open_fold_starting_at(range.start_line);
                }
                if let Some(new_cursor) =
                    self.insert_auto_indented_lines_before(insert_before_line, 1, prefix)
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Some(EditorOperation::ToggleLineComment) => self.handle_count_toggle_line_comment(1),
            Some(EditorOperation::MoveToMatchingBracket) => {
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
            Some(EditorOperation::MoveToPreviousParagraph) => {
                self.move_cursor_to_previous_paragraph();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToNextParagraph) => {
                self.move_cursor_to_next_paragraph();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToPreviousDiffHunk) => {
                self.move_cursor_to_previous_diff_hunk();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToNextDiffHunk) => {
                self.move_cursor_to_next_diff_hunk();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToPreviousDiffHunkEnd) => {
                self.move_cursor_to_previous_diff_hunk_end();
                ActionResult::Handled
            }
            Some(EditorOperation::MoveToNextDiffHunkEnd) => {
                self.move_cursor_to_next_diff_hunk_end();
                ActionResult::Handled
            }
            Some(EditorOperation::FindForward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_to_char_forward(*target, 1);
                ActionResult::Handled
            }
            Some(EditorOperation::FindBackward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Find,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_to_char_backward(*target, 1);
                ActionResult::Handled
            }
            Some(EditorOperation::TillForward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Forward,
                });
                self.move_cursor_till_forward(*target, 1);
                ActionResult::Handled
            }
            Some(EditorOperation::TillBackward(target)) => {
                globals::set_last_find(globals::FindState {
                    target_char: *target,
                    kind: globals::FindKind::Till,
                    direction: globals::Direction::Backward,
                });
                self.move_cursor_till_backward(*target, 1);
                ActionResult::Handled
            }
            Some(EditorOperation::RepeatLastFind) => {
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
            Some(EditorOperation::RepeatLastFindReverse) => {
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
            Some(EditorOperation::VisualTextObject(text_object)) => {
                if action.from_mode != Some(ModeKind::Visual) {
                    ActionResult::NotHandled
                } else {
                    self.select_visual_text_object(*text_object, 1)
                }
            }
            Some(EditorOperation::SurroundReplace {
                target,
                replacement,
            }) => self.replace_surround(*target, *replacement),
            Some(EditorOperation::ReplaceChar(c)) => {
                self.replace_char_at_cursor(*c);
                ActionResult::Handled
            }
            Some(EditorOperation::ReplaceBackspaceLast) => self.restore_last_replace_char(),
            Some(EditorOperation::ReplaceBackspace {
                cursor,
                replaced,
                inserted,
            }) => {
                self.restore_replace_char(*cursor, *replaced, *inserted);
                ActionResult::Handled
            }
            Some(EditorOperation::SurroundDelete { target }) => self.delete_surround(*target),
            Some(EditorOperation::SurroundAdd { target, delimiter }) => {
                self.add_surround(*target, *delimiter)
            }
            Some(EditorOperation::SurroundAddSelection { delimiter }) => {
                self.add_surround_selection(*delimiter, action.from_mode)
            }
            Some(EditorOperation::Count(count, inner)) => {
                let mut counted_inner = inner.as_ref().clone();
                if let Some(from_mode) = action.from_mode {
                    counted_inner = counted_inner.with_from_mode(from_mode);
                }
                self.handle_count(*count, &counted_inner)
            }
            Some(EditorOperation::Operation(op, target)) => {
                return self.handle_operation(op, target, action.from_mode, action.register);
            }
            Some(EditorOperation::Undo)
            | Some(EditorOperation::Redo)
            | Some(EditorOperation::RepeatLastChange)
            | Some(EditorOperation::JumpBackward)
            | Some(EditorOperation::JumpForward)
            | None => ActionResult::NotHandled,
        };

        if result == ActionResult::Handled && action.from_mode != Some(ModeKind::Insert) {
            let mode = action
                .to_mode
                .unwrap_or(action.from_mode.unwrap_or_else(|| self.mode_kind()));
            self.clamp_cursor_for_mode(mode);
        }

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
