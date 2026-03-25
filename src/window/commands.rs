use super::*;

impl Window {
    pub fn insert_char(&mut self, c: char) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        buffer.insert_char(cursor, c);
        let new_cursor = match c {
            '\n' => Cursor::new(cursor.line + 1, 0),
            _ => Cursor::new(cursor.line, cursor.col + c.len_utf8()),
        };
        self.buffer_view.set_cursor(new_cursor);
    }

    pub fn delete_char_before_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.delete_char_before_cursor(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn delete_char_at_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.delete_char_at_cursor(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn join_lines_with_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.join_lines(cursor.line, 2, true) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn join_lines_without_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.join_lines(cursor.line, 2, false) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub(crate) fn handle_count(&mut self, count: usize, inner: &Action) -> ActionResult {
        match inner {
            Action::MoveToFirstLine | Action::MoveToLastLine => {
                self.handle_count_line_motion(count, inner)
            }
            Action::MoveToScreenTop | Action::MoveToScreenBottom => {
                self.handle_count_screen_motion(count, inner)
            }
            Action::JoinWithSpace | Action::JoinWithoutSpace => {
                self.handle_count_join(count, inner)
            }
            Action::DeleteLine => self.handle_count_delete_line(count),
            Action::ChangeLine => self.handle_count_change_line(count),
            Action::ChangeToLineEnd => self.handle_count_change_to_line_end(count),
            Action::OpenLineBelow => self.handle_count_open_line_below(count),
            Action::OpenLineAbove => self.handle_count_open_line_above(count),
            Action::Operation(_, _) => self.handle_count_operation(count, inner),
            _ if inner.is_line_action() => self.handle_count_line_action(count, inner),
            _ => self.handle_count_repeatable(count, inner),
        }
    }

    fn handle_count_operation(&mut self, count: usize, action: &Action) -> ActionResult {
        if let Action::Operation(op, target) = action {
            return self.handle_operation_with_count(*op, *target, count);
        }
        ActionResult::Handled
    }

    fn handle_count_line_motion(&mut self, count: usize, _action: &Action) -> ActionResult {
        let line_count = self.buffer_view.buffer.line_count();
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
        let line_count = self.buffer_view.buffer().line_count();
        if line_count == 0 {
            return ActionResult::Handled;
        }
        let target_line = if matches!(action, Action::MoveToScreenTop) {
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
        let with_space = matches!(action, Action::JoinWithSpace);
        let cursor = self.buffer_view.cursor();
        let actual_count = count + 1;
        if let Some(new_cursor) =
            self.buffer_view
                .buffer
                .join_lines(cursor.line, actual_count, with_space)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_delete_line(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.delete_lines(cursor.line, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_change_line(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.change_lines(cursor.line, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    pub(super) fn handle_count_change_to_line_end(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.change_to_line_end(cursor, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_open_line_below(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .buffer
            .insert_lines_after(cursor.line, count)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .buffer
            .insert_lines_before(cursor.line, count)
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
        let buffer = self.buffer_view.buffer_mut();
        let range = buffer.get_operator_target_range_with_count(cursor, target, count);
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };
        if range.start == range.end {
            return self.operation_noop_result(operator);
        }
        if let Some(new_cursor) = buffer.delete_range(range) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_linewise_operation_with_count(
        &mut self,
        operator: Operator,
        motion: LinewiseMotion,
        count: usize,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        let range = buffer.get_linewise_operator_target_range_with_count(cursor, motion, count);
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };
        if range.count == 0 {
            return self.operation_noop_result(operator);
        }
        if let Some(new_cursor) = buffer.delete_lines(range.start_line, range.count) {
            self.buffer_view.set_cursor(new_cursor);
        }
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
        let buffer = self.buffer_view.buffer_mut();
        let range = buffer.get_linewise_operator_target_range(cursor, motion);
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };
        if range.count == 0 {
            return self.operation_noop_result(operator);
        }
        if let Some(new_cursor) = buffer.delete_lines(range.start_line, range.count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn operation_noop_result(&self, operator: Operator) -> ActionResult {
        match operator {
            Operator::Delete => ActionResult::Handled,
            Operator::Change => ActionResult::NotHandled,
        }
    }
}
