use super::*;
use crate::editor::ActionKind;
use crate::editor::pairs;

impl Window {
    pub fn insert_char(&mut self, c: char) {
        let cursor = self.buffer_view.cursor();
        self.buffer_view
            .with_buffer_mut(|buffer| buffer.insert_char(cursor, c))
            .unwrap_or(());
        let new_cursor = match c {
            '\n' => Cursor::new(cursor.line + 1, 0),
            _ => Cursor::new(cursor.line, cursor.col + c.len_utf8()),
        };
        self.buffer_view.set_cursor(new_cursor);
    }

    /// Inserts a supported delimiter pair and places the cursor between the two characters.
    pub fn insert_pair(&mut self, opening: char, closing: char) {
        let cursor = self.buffer_view.cursor();
        let pair = [opening, closing].into_iter().collect::<String>();
        self.buffer_view
            .with_buffer_mut(|buffer| buffer.insert_text(cursor, &pair))
            .unwrap_or(());
        let new_cursor = Cursor::new(cursor.line, cursor.col + opening.len_utf8());
        self.buffer_view.set_cursor(new_cursor);
    }

    pub(super) fn auto_indent_enabled(&self) -> bool {
        globals::with_config(|config| config.auto_indent)
            .unwrap_or_default()
            != crate::config::AutoIndentMode::Off
    }

    pub(super) fn inferred_newline_prefix(&self, cursor: Cursor) -> Option<String> {
        if !self.auto_indent_enabled() {
            return None;
        }

        self.buffer_view
            .with_buffer(|buffer| buffer.inferred_auto_indent_prefix(cursor))
            .flatten()
    }

    pub(super) fn insert_newline(&mut self) -> Option<String> {
        let cursor = self.buffer_view.cursor();
        let prefix = self.inferred_newline_prefix(cursor);
        self.insert_char('\n');

        if let Some(prefix) = prefix.as_deref() {
            if let Some(new_cursor) =
                self.insert_prefix_on_line_range(cursor.line + 1, 1, prefix)
            {
                self.buffer_view.set_cursor(new_cursor);
            }
        }

        prefix
    }

    pub(super) fn current_line_indentation(&self) -> Option<String> {
        let cursor = self.buffer_view.cursor();
        self.buffer_view
            .with_buffer(|buffer| buffer.line_leading_whitespace_prefix(cursor.line))
            .flatten()
    }

    pub(super) fn change_lines_with_auto_indent(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let line_indentation = if self.auto_indent_enabled() {
            self.current_line_indentation()
        } else {
            None
        };

        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.change_lines(cursor.line, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }

        if let Some(indentation) = line_indentation.filter(|indentation| !indentation.is_empty())
        {
            self.pending_repeat_suffix = Some(indentation.clone());
            if let Some(new_cursor) =
                self.insert_prefix_on_line_range(cursor.line, 1, &indentation)
            {
                self.buffer_view.set_cursor(new_cursor);
            }
        }

        ActionResult::Handled
    }

    pub(super) fn insert_prefix_on_line_range(
        &mut self,
        start_line: usize,
        count: usize,
        prefix: &str,
    ) -> Option<Cursor> {
        if prefix.is_empty() {
            return Some(Cursor::new(start_line, 0));
        }

        let end_line = start_line.saturating_add(count);
        self.buffer_view.with_buffer_mut(|buffer| {
            for line_idx in start_line..end_line {
                buffer.insert_text(Cursor::new(line_idx, 0), prefix);
            }
        })?;

        Some(Cursor::new(start_line, prefix.len()))
    }

    pub(super) fn insert_auto_indented_lines_after(
        &mut self,
        line: usize,
        count: usize,
        prefix: Option<String>,
    ) -> Option<Cursor> {
        let new_cursor = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.insert_lines_after(line, count))
            .flatten()?;

        match prefix.as_deref() {
            Some(prefix) => self.insert_prefix_on_line_range(new_cursor.line, count, prefix),
            None => Some(new_cursor),
        }
    }

    pub(super) fn insert_auto_indented_lines_before(
        &mut self,
        line: usize,
        count: usize,
        prefix: Option<String>,
    ) -> Option<Cursor> {
        let new_cursor = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.insert_lines_before(line, count))
            .flatten()?;

        match prefix.as_deref() {
            Some(prefix) => self.insert_prefix_on_line_range(new_cursor.line, count, prefix),
            None => Some(new_cursor),
        }
    }

    pub fn delete_char_before_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_char_before_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Deletes the character before the cursor, or a matching auto-closed pair in insert mode.
    pub fn delete_insert_char_before_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        let should_delete_pair =
            crate::globals::with_config(|config| config.auto_close_pairs).unwrap_or(true);
        if should_delete_pair
            && let Some((start, end)) = self
                .buffer_view
                .with_buffer(|buffer| {
                    let opening = buffer.char_before_cursor(cursor)?;
                    let closing = buffer.char_at_cursor(cursor)?;
                    if pairs::closer_for(opening) != Some(closing) {
                        return None;
                    }
                    let start = buffer.prev_cursor_line(cursor)?;
                    let end = buffer.next_cursor(cursor)?;
                    Some((start, end))
                })
                .flatten()
        {
            self.buffer_view
                .with_buffer_mut(|buffer| buffer.remove(start, end))
                .unwrap_or(());
            self.buffer_view.set_cursor(start);
            return;
        }

        self.delete_char_before_cursor();
    }

    pub fn delete_char_at_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_char_at_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn join_lines_with_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.join_lines(cursor.line, 2, true))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn join_lines_without_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.join_lines(cursor.line, 2, false))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub(crate) fn handle_count(&mut self, count: usize, inner: &Action) -> ActionResult {
        match inner.kind.as_ref() {
            Some(ActionKind::MoveToFirstLine) | Some(ActionKind::MoveToLastLine) => {
                self.handle_count_line_motion(count, inner)
            }
            Some(ActionKind::MoveToScreenTop) | Some(ActionKind::MoveToScreenBottom) => {
                self.handle_count_screen_motion(count, inner)
            }
            Some(ActionKind::JoinWithSpace) | Some(ActionKind::JoinWithoutSpace) => {
                self.handle_count_join(count, inner)
            }
            Some(ActionKind::DeleteLine) => self.handle_count_delete_line(count),
            Some(ActionKind::ChangeLine) => self.handle_count_change_line(count),
            Some(ActionKind::ChangeToLineEnd) => self.handle_count_change_to_line_end(count),
            Some(ActionKind::OpenLineBelow) => self.handle_count_open_line_below(count),
            Some(ActionKind::OpenLineAbove) => self.handle_count_open_line_above(count),
            Some(ActionKind::ToggleLineComment) => self.handle_count_toggle_line_comment(count),
            Some(ActionKind::Operation(_, _)) => self.handle_count_operation(count, inner),
            _ if inner.is_line_action() => self.handle_count_line_action(count, inner),
            _ => self.handle_count_repeatable(count, inner),
        }
    }

    fn handle_count_operation(&mut self, count: usize, action: &Action) -> ActionResult {
        if let Some(ActionKind::Operation(op, target)) = action.kind.as_ref() {
            return self.handle_operation_with_count(*op, *target, count);
        }
        ActionResult::Handled
    }

    fn handle_count_line_motion(&mut self, count: usize, _action: &Action) -> ActionResult {
        let line_count = self.buffer_view.line_count();
        if line_count == 0 {
            return ActionResult::Handled;
        }
        let target_line = (count - 1).min(line_count - 1);
        let target_col = self.buffer_view.get_or_compute_target_col();
        self.set_cursor_to_visual_col_on_line(target_line, target_col);
        ActionResult::Handled
    }

    fn handle_count_screen_motion(&mut self, count: usize, action: &Action) -> ActionResult {
        let viewport_rows = self.size.rows as usize;
        if viewport_rows == 0 {
            return ActionResult::Handled;
        }
        let start_line = self.buffer_view.scroll_offset().row as usize;
        let line_count = self.buffer_view.line_count();
        if line_count == 0 {
            return ActionResult::Handled;
        }
        let target_line = if matches!(action.kind.as_ref(), Some(ActionKind::MoveToScreenTop)) {
            let offset = count.saturating_sub(1);
            (start_line + offset)
                .min(start_line + viewport_rows - 1)
                .min(line_count - 1)
        } else {
            let end_line = (start_line + viewport_rows - 1).min(line_count - 1);
            let offset = count.saturating_sub(1);
            end_line.saturating_sub(offset).max(start_line)
        };
        let target_col = self.buffer_view.get_or_compute_target_col();
        self.set_cursor_to_visual_col_on_line(target_line, target_col);
        ActionResult::Handled
    }

    fn handle_count_line_action(&mut self, count: usize, action: &Action) -> ActionResult {
        let target_line = (count as isize - 1).max(0) as usize;
        let current_cursor = self.buffer_view.cursor();
        self.buffer_view
            .set_cursor(Cursor::new(target_line, current_cursor.col));
        self.process_action(action)
    }

    fn handle_count_join(&mut self, count: usize, action: &Action) -> ActionResult {
        let with_space = matches!(action.kind.as_ref(), Some(ActionKind::JoinWithSpace));
        let cursor = self.buffer_view.cursor();
        let actual_count = count + 1;
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.join_lines(cursor.line, actual_count, with_space))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_delete_line(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_lines(cursor.line, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_change_line(&mut self, count: usize) -> ActionResult {
        self.change_lines_with_auto_indent(count)
    }

    pub(super) fn handle_count_change_to_line_end(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.change_to_line_end(cursor, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    pub(super) fn handle_count_toggle_line_comment(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let Some(comment_prefix) = self
            .buffer_view
            .with_buffer(|buffer| buffer.comment_prefix())
            .flatten()
        else {
            return ActionResult::NotHandled;
        };

        let line_count = self.buffer_view.line_count();
        if cursor.line >= line_count {
            return ActionResult::NotHandled;
        }

        let actual_count = count.min(line_count.saturating_sub(cursor.line));
        if actual_count == 0 {
            return ActionResult::Handled;
        }

        let new_cursor = self
            .buffer_view
            .with_buffer_mut(|buffer| {
                buffer.toggle_line_comments(cursor, actual_count, &comment_prefix)
            })
            .flatten();

        if let Some(new_cursor) = new_cursor {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_open_line_below(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let prefix = self.inferred_newline_prefix(cursor);
        if let Some(new_cursor) =
            self.insert_auto_indented_lines_after(cursor.line, count, prefix)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let prefix = self.inferred_newline_prefix(cursor);
        if let Some(new_cursor) =
            self.insert_auto_indented_lines_before(cursor.line, count, prefix)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_repeatable(&mut self, count: usize, action: &Action) -> ActionResult {
        for _ in 0..count {
            self.process_action(action);
        }
        ActionResult::Handled
    }

    fn handle_operation_with_count(
        &mut self,
        operator: Operator,
        target: OperatorTarget,
        count: usize,
    ) -> ActionResult {
        match target {
            OperatorTarget::LinewiseMotion(motion) => {
                self.handle_linewise_operation_with_count(operator, motion, count)
            }
            _ => self.handle_characterwise_operation_with_count(operator, target, count),
        }
    }

    fn handle_characterwise_operation_with_count(
        &mut self,
        operator: Operator,
        target: OperatorTarget,
        count: usize,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let result = self.buffer_view.with_buffer_mut(|buffer| {
            let range = buffer.get_operator_target_range_with_count(cursor, target, count);
            let range = range?;
            if range.start == range.end {
                return match operator {
                    Operator::Change => Some(range.start),
                    Operator::Delete => None,
                };
            }
            buffer.delete_range(range)
        });
        let Some(Some(new_cursor)) = result else {
            return self.operation_noop_result(operator);
        };
        self.buffer_view.set_cursor(new_cursor);
        ActionResult::Handled
    }

    fn handle_linewise_operation_with_count(
        &mut self,
        operator: Operator,
        motion: LinewiseMotion,
        count: usize,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let result = self.buffer_view.with_buffer_mut(|buffer| {
            let range = buffer.get_linewise_operator_target_range_with_count(cursor, motion, count);
            let range = range?;
            if range.count == 0 {
                return None;
            }
            buffer.delete_lines(range.start_line, range.count)
        });
        let Some(Some(new_cursor)) = result else {
            return self.operation_noop_result(operator);
        };
        self.buffer_view.set_cursor(new_cursor);
        ActionResult::Handled
    }

    pub(super) fn handle_operation(
        &mut self,
        operator: &Operator,
        target: &OperatorTarget,
    ) -> ActionResult {
        match target {
            OperatorTarget::LinewiseMotion(motion) => {
                self.handle_linewise_operation(*operator, *motion)
            }
            _ => self.handle_characterwise_operation_with_count(*operator, *target, 1),
        }
    }

    fn handle_linewise_operation(
        &mut self,
        operator: Operator,
        motion: LinewiseMotion,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let result = self.buffer_view.with_buffer_mut(|buffer| {
            let range = buffer.get_linewise_operator_target_range(cursor, motion);
            let range = range?;
            if range.count == 0 {
                return None;
            }
            buffer.delete_lines(range.start_line, range.count)
        });
        let Some(Some(new_cursor)) = result else {
            return self.operation_noop_result(operator);
        };
        self.buffer_view.set_cursor(new_cursor);
        ActionResult::Handled
    }

    fn operation_noop_result(&self, operator: Operator) -> ActionResult {
        match operator {
            Operator::Delete => ActionResult::Handled,
            Operator::Change => ActionResult::NotHandled,
        }
    }
}
