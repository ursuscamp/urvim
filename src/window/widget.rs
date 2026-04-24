use super::*;
use crate::editor::pairs;
use crate::editor::{ActionKind, ModeKind};
use crate::register::{DefaultRegisterRole, RegisterContentKind};

impl Window {
    /// Dispatches an editor action to this window.
    pub fn dispatch_action(&mut self, action: &Action) -> ActionResult {
        self.pending_repeat_suffix = None;
        let insert_mode = action.from_mode == Some(ModeKind::Insert);
        let result =
            match action.kind.as_ref() {
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
                Some(ActionKind::MovePageUp) => {
                    self.move_cursor_page_up(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::MovePageDown) => {
                    self.move_cursor_page_down(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::MoveHalfPageUp) => {
                    self.move_cursor_half_page_up(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::MoveHalfPageDown) => {
                    self.move_cursor_half_page_down(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::MoveRight) => {
                    self.move_cursor_right();
                    ActionResult::Handled
                }
                Some(ActionKind::InsertChar(c)) => {
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
                Some(ActionKind::InsertText(text)) => {
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
                Some(ActionKind::InsertRawPaste(text)) => {
                    if self.insert_raw_text(text).is_some() {
                        ActionResult::Handled
                    } else {
                        ActionResult::NotHandled
                    }
                }
                Some(ActionKind::ReplaceSelectionRawPaste(text)) => {
                    if self
                        .replace_visual_selection_with_raw_text(text, action.from_mode)
                        .is_some()
                    {
                        ActionResult::Handled
                    } else {
                        ActionResult::NotHandled
                    }
                }
                Some(ActionKind::InsertNewline) => {
                    self.pending_repeat_suffix = self.insert_newline();
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
                    let target_line =
                        start_line.min(self.buffer_view.line_count().saturating_sub(1));
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
                Some(ActionKind::ViewportCursorTop) => {
                    self.align_viewport_cursor_top(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::ViewportCursorCenter) => {
                    self.align_viewport_cursor_center(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::ViewportCursorBottom) => {
                    self.align_viewport_cursor_bottom(self.size.rows as usize);
                    ActionResult::Handled
                }
                Some(ActionKind::DeleteBackward) => {
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
                Some(ActionKind::IndentDecrease) => {
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
                Some(ActionKind::IndentIncrease) => {
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
                Some(ActionKind::DeleteForward) => {
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
                Some(ActionKind::DeleteSelection) => {
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
                        let content = self.buffer_view.visual_selection_range().and_then(|range| {
                            self.capture_characterwise_text(range.start, range.end)
                        });
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
                Some(ActionKind::YankSelection) => {
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
                        let content = self.buffer_view.visual_selection_range().and_then(|range| {
                            self.capture_characterwise_text(range.start, range.end)
                        });
                        self.store_register_text(
                            action.register,
                            DefaultRegisterRole::Yank,
                            content,
                            RegisterContentKind::Characterwise,
                        );
                    }
                    ActionResult::Handled
                }
                Some(ActionKind::YankLine) => {
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
                Some(ActionKind::ChangeLine) => {
                    let cursor = self.buffer_view.cursor();
                    self.store_register_text(
                        action.register,
                        DefaultRegisterRole::Change,
                        self.capture_linewise_text(cursor.line, 1),
                        RegisterContentKind::Linewise,
                    );
                    self.change_lines_with_auto_indent(1)
                }
                Some(ActionKind::ChangeSelection) => {
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
                        let content = self.buffer_view.visual_selection_range().and_then(|range| {
                            self.capture_characterwise_text(range.start, range.end)
                        });
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
                Some(ActionKind::ChangeToLineEnd) => {
                    self.handle_count_change_to_line_end(1, action.register);
                    ActionResult::Handled
                }
                Some(ActionKind::PasteAfter) => {
                    self.paste_register_content(action.register, DefaultRegisterRole::Yank, true)
                }
                Some(ActionKind::PasteBefore) => {
                    self.paste_register_content(action.register, DefaultRegisterRole::Yank, false)
                }
                Some(ActionKind::OpenLineBelow) => {
                    let cursor = self.buffer_view.cursor();
                    let prefix = self.inferred_newline_prefix(cursor);
                    if let Some(new_cursor) =
                        self.insert_auto_indented_lines_after(cursor.line, 1, prefix)
                    {
                        self.buffer_view.set_cursor(new_cursor);
                    }
                    ActionResult::Handled
                }
                Some(ActionKind::OpenLineAbove) => {
                    let cursor = self.buffer_view.cursor();
                    let prefix = self.inferred_newline_prefix(cursor);
                    if let Some(new_cursor) =
                        self.insert_auto_indented_lines_before(cursor.line, 1, prefix)
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
                Some(ActionKind::ToggleWrap) => {
                    self.toggle_wrap();
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
                Some(ActionKind::VisualTextObject(text_object)) => {
                    if action.from_mode != Some(ModeKind::Visual) {
                        ActionResult::NotHandled
                    } else {
                        self.select_visual_text_object(*text_object, 1)
                    }
                }
                Some(ActionKind::SurroundReplace {
                    target,
                    replacement,
                }) => self.replace_surround(*target, *replacement),
                Some(ActionKind::SurroundDelete { target }) => self.delete_surround(*target),
                Some(ActionKind::SurroundAdd { target, delimiter }) => {
                    self.add_surround(*target, *delimiter)
                }
                Some(ActionKind::SurroundAddSelection { delimiter }) => {
                    self.add_surround_selection(*delimiter, action.from_mode)
                }
                Some(ActionKind::Count(count, inner)) => {
                    let mut counted_inner = inner.as_ref().clone();
                    if let Some(from_mode) = action.from_mode {
                        counted_inner = counted_inner.with_from_mode(from_mode);
                    }
                    self.handle_count(*count, &counted_inner)
                }
                Some(ActionKind::Operation(op, target)) => {
                    return self.handle_operation(op, target, action.from_mode, action.register);
                }
                Some(ActionKind::Quit)
                | Some(ActionKind::Undo)
                | Some(ActionKind::Redo)
                | Some(ActionKind::SaveBuffer(_))
                | Some(ActionKind::RepeatLastChange)
                | Some(ActionKind::JumpBackward)
                | Some(ActionKind::JumpForward)
                | Some(ActionKind::PreviousTab)
                | Some(ActionKind::NextTab)
                | Some(ActionKind::SplitVertical)
                | Some(ActionKind::SplitHorizontal)
                | Some(ActionKind::FocusPaneLeft)
                | Some(ActionKind::FocusPaneDown)
                | Some(ActionKind::FocusPaneUp)
                | Some(ActionKind::FocusPaneRight)
                | Some(ActionKind::ResizePaneLeft)
                | Some(ActionKind::ResizePaneRight)
                | Some(ActionKind::ResizePaneUp)
                | Some(ActionKind::ResizePaneDown)
                | Some(ActionKind::EqualizeSplits)
                | Some(ActionKind::ClosePane)
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
