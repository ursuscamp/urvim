use super::*;
use crate::buffer::IndentDirection;
use crate::config::DefaultRegisters;
use crate::editor::ActionKind;
use crate::editor::pairs;
use crate::register::{
    self, DefaultRegisterRole, RegisterContent, RegisterContentKind, RegisterName,
};

impl Window {
    fn resolved_register_name(
        explicit: Option<RegisterName>,
        role: DefaultRegisterRole,
    ) -> RegisterName {
        explicit.unwrap_or_else(|| {
            let defaults = globals::with_config(|config| config.default_registers.clone())
                .unwrap_or_else(DefaultRegisters::default);
            register::default_register_name(role, &defaults)
        })
    }

    pub(super) fn store_register_content(
        &self,
        explicit: Option<RegisterName>,
        role: DefaultRegisterRole,
        text: String,
        kind: RegisterContentKind,
    ) {
        let register = Self::resolved_register_name(explicit, role);
        globals::with_register_store_mut(|store| {
            store.set(register, RegisterContent::new(text, kind));
        });
    }

    pub(super) fn store_register_text(
        &self,
        explicit: Option<RegisterName>,
        role: DefaultRegisterRole,
        text: Option<String>,
        kind: RegisterContentKind,
    ) {
        if let Some(text) = text {
            self.store_register_content(explicit, role, text, kind);
        }
    }

    pub(super) fn capture_characterwise_text(&self, start: Cursor, end: Cursor) -> Option<String> {
        self.buffer_view
            .with_buffer(|buffer| buffer.text_in_range(start, end))
            .flatten()
    }

    pub(super) fn capture_linewise_text(&self, start_line: usize, count: usize) -> Option<String> {
        self.buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(start_line, count))
            .flatten()
    }

    fn paste_characterwise_text(&mut self, text: &str, after: bool) -> Option<Cursor> {
        let cursor = self.buffer_view.cursor();
        let insert_cursor = if after {
            self.buffer_view
                .with_buffer(|buffer| buffer.next_cursor_line(cursor))
                .flatten()
                .unwrap_or(cursor)
        } else {
            cursor
        };

        self.buffer_view
            .with_buffer_mut(|buffer| buffer.insert_text(insert_cursor, text))
            .unwrap_or(());

        let mut new_cursor = insert_cursor;
        for ch in text.chars() {
            if ch == '\n' {
                new_cursor = Cursor::new(new_cursor.line + 1, 0);
            } else {
                new_cursor = Cursor::new(new_cursor.line, new_cursor.col + ch.len_utf8());
            }
        }

        Some(new_cursor)
    }

    fn paste_linewise_text(&mut self, text: &str, after: bool) -> Option<Cursor> {
        let cursor = self.buffer_view.cursor();
        let lines: Vec<&str> = text.split('\n').collect();
        let line_count = lines.len();
        let start_cursor = if after {
            self.insert_auto_indented_lines_after(cursor.line, line_count, None)?
        } else {
            self.insert_auto_indented_lines_before(cursor.line, line_count, None)?
        };

        self.buffer_view.with_buffer_mut(|buffer| {
            for (offset, line) in lines.iter().enumerate() {
                buffer.insert_text(Cursor::new(start_cursor.line + offset, 0), line);
            }
        })?;

        Some(start_cursor)
    }

    pub(super) fn paste_register_content(
        &mut self,
        explicit: Option<RegisterName>,
        role: DefaultRegisterRole,
        after: bool,
    ) -> ActionResult {
        let register = Self::resolved_register_name(explicit, role);
        let Some(content) = globals::with_register_store(|store| store.get(register)) else {
            return ActionResult::Handled;
        };

        let new_cursor = match content.kind {
            RegisterContentKind::Characterwise => {
                self.paste_characterwise_text(&content.text, after)
            }
            RegisterContentKind::Linewise => self.paste_linewise_text(&content.text, after),
        };

        if let Some(new_cursor) = new_cursor {
            self.buffer_view.set_cursor(new_cursor);
        }

        ActionResult::Handled
    }

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
        globals::with_config(|config| config.auto_indent).unwrap_or_default()
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
            if let Some(new_cursor) = self.insert_prefix_on_line_range(cursor.line + 1, 1, prefix) {
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

        if let Some(indentation) = line_indentation.filter(|indentation| !indentation.is_empty()) {
            self.pending_repeat_suffix = Some(indentation.clone());
            if let Some(new_cursor) = self.insert_prefix_on_line_range(cursor.line, 1, &indentation)
            {
                self.buffer_view.set_cursor(new_cursor);
            }
        }

        ActionResult::Handled
    }

    pub(super) fn shift_lines_indentation(
        &mut self,
        start_line: usize,
        count: usize,
        direction: IndentDirection,
    ) -> Option<Cursor> {
        let cursor = self.buffer_view.cursor();
        let line_count = self.buffer_view.line_count();
        if line_count == 0 || start_line >= line_count || count == 0 {
            return Some(cursor);
        }

        let actual_count = (line_count - start_line).min(count);
        let mut cursor_delta = None;

        for line_idx in start_line..start_line + actual_count {
            let delta = self
                .buffer_view
                .with_buffer_mut(|buffer| buffer.shift_line_indentation(line_idx, direction))
                .flatten()?;
            if line_idx == cursor.line {
                cursor_delta = Some(delta);
            }
        }

        if let Some(delta) = cursor_delta {
            let new_cursor = match direction {
                IndentDirection::Increase => Cursor::new(cursor.line, cursor.col + delta),
                IndentDirection::Decrease => {
                    Cursor::new(cursor.line, cursor.col.saturating_sub(delta))
                }
            };
            self.buffer_view.set_cursor(new_cursor);
            return Some(new_cursor);
        }

        Some(cursor)
    }

    pub(super) fn delete_insert_indent_before_cursor(&mut self) -> bool {
        let cursor = self.buffer_view.cursor();
        let Some(prefix_len) = self
            .buffer_view
            .with_buffer(|buffer| {
                buffer
                    .line_leading_whitespace_prefix(cursor.line)
                    .map(|prefix| prefix.len())
            })
            .flatten()
        else {
            return false;
        };

        if prefix_len == 0 || cursor.col > prefix_len {
            return false;
        }

        if let Some(new_cursor) =
            self.shift_lines_indentation(cursor.line, 1, IndentDirection::Decrease)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        true
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

    /// Deletes the active visual selection and leaves the cursor at the selection start.
    pub fn delete_visual_selection(&mut self) {
        let Some(range) = self.buffer_view.visual_selection_range() else {
            return;
        };

        self.buffer_view
            .with_buffer_mut(|buffer| buffer.delete_range(range))
            .flatten();
        self.buffer_view.set_cursor(range.start);
    }

    /// Deletes the active linewise visual selection and leaves the cursor at the selection start.
    pub fn delete_linewise_visual_selection(&mut self) {
        let Some((start_line, count)) = self.buffer_view.visual_line_selection_range() else {
            return;
        };

        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_lines(start_line, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Changes the active visual selection and leaves the cursor at the selection start.
    pub fn change_visual_selection(&mut self) {
        self.delete_visual_selection();
    }

    /// Changes the active linewise visual selection and leaves the cursor at the replacement line.
    pub fn change_linewise_visual_selection(&mut self) {
        let Some((start_line, count)) = self.buffer_view.visual_line_selection_range() else {
            return;
        };

        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.change_lines(start_line, count))
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
            Some(ActionKind::DeleteLine) => self.handle_count_delete_line(count, inner.register),
            Some(ActionKind::YankLine) => self.handle_count_yank_line(count, inner.register),
            Some(ActionKind::ChangeLine) => self.handle_count_change_line(count, inner.register),
            Some(ActionKind::ChangeToLineEnd) => {
                self.handle_count_change_to_line_end(count, inner.register)
            }
            Some(ActionKind::OpenLineBelow) => self.handle_count_open_line_below(count),
            Some(ActionKind::OpenLineAbove) => self.handle_count_open_line_above(count),
            Some(ActionKind::IndentDecrease) => {
                let cursor = self.buffer_view.cursor();
                self.shift_lines_indentation(cursor.line, count, IndentDirection::Decrease);
                ActionResult::Handled
            }
            Some(ActionKind::IndentIncrease) => {
                let cursor = self.buffer_view.cursor();
                self.shift_lines_indentation(cursor.line, count, IndentDirection::Increase);
                ActionResult::Handled
            }
            Some(ActionKind::ToggleLineComment) => self.handle_count_toggle_line_comment(count),
            Some(ActionKind::Operation(_, _)) => self.handle_count_operation(count, inner),
            _ if inner.is_line_action() => self.handle_count_line_action(count, inner),
            _ => self.handle_count_repeatable(count, inner),
        }
    }

    fn handle_count_operation(&mut self, count: usize, action: &Action) -> ActionResult {
        if let Some(ActionKind::Operation(op, target)) = action.kind.as_ref() {
            return self.handle_operation_with_count(
                *op,
                *target,
                count,
                action.from_mode,
                action.register,
            );
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

    fn handle_count_delete_line(
        &mut self,
        count: usize,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(cursor.line, count))
            .flatten()
        {
            self.store_register_content(
                register,
                DefaultRegisterRole::Delete,
                text,
                RegisterContentKind::Linewise,
            );
        }
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_lines(cursor.line, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_change_line(
        &mut self,
        count: usize,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(cursor.line, count))
            .flatten()
        {
            self.store_register_content(
                register,
                DefaultRegisterRole::Change,
                text,
                RegisterContentKind::Linewise,
            );
        }
        self.change_lines_with_auto_indent(count)
    }

    pub(super) fn handle_count_change_to_line_end(
        &mut self,
        count: usize,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| {
                let total_lines = buffer.line_count();
                let end_line = cursor
                    .line
                    .saturating_add(count.saturating_sub(1))
                    .min(total_lines.saturating_sub(1));
                let end = Cursor::new(end_line, buffer.line_len(end_line));
                buffer.text_in_range(cursor, end)
            })
            .flatten()
        {
            self.store_register_content(
                register,
                DefaultRegisterRole::Change,
                text,
                RegisterContentKind::Characterwise,
            );
        }
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.change_to_line_end(cursor, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_yank_line(
        &mut self,
        count: usize,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(cursor.line, count))
            .flatten()
        {
            self.store_register_content(
                register,
                DefaultRegisterRole::Yank,
                text,
                RegisterContentKind::Linewise,
            );
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
        if let Some(new_cursor) = self.insert_auto_indented_lines_after(cursor.line, count, prefix)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let prefix = self.inferred_newline_prefix(cursor);
        if let Some(new_cursor) = self.insert_auto_indented_lines_before(cursor.line, count, prefix)
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

    fn transform_case_text(text: &str, operator: Operator) -> String {
        let mut transformed = String::with_capacity(text.len());
        for ch in text.chars() {
            match operator {
                Operator::Lowercase => transformed.extend(ch.to_lowercase()),
                Operator::Uppercase => transformed.extend(ch.to_uppercase()),
                Operator::ToggleCase => {
                    if ch.is_lowercase() {
                        transformed.extend(ch.to_uppercase());
                    } else if ch.is_uppercase() {
                        transformed.extend(ch.to_lowercase());
                    } else {
                        transformed.push(ch);
                    }
                }
                Operator::Delete | Operator::Change | Operator::Yank => transformed.push(ch),
            }
        }
        transformed
    }

    fn handle_case_selection(
        &mut self,
        operator: Operator,
        from_mode: Option<ModeKind>,
    ) -> ActionResult {
        match from_mode {
            Some(ModeKind::VisualLine) => {
                let Some((start_line, count)) = self.buffer_view.visual_line_selection_range()
                else {
                    return self.operation_noop_result(operator);
                };
                let Some(text) = self.capture_linewise_text(start_line, count) else {
                    return self.operation_noop_result(operator);
                };
                let transformed = Self::transform_case_text(&text, operator);
                let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| {
                        let cursor = buffer.delete_lines(start_line, count)?;
                        buffer.insert_text(Cursor::new(start_line, 0), &transformed);
                        Some(cursor)
                    })
                    .flatten()
                else {
                    return self.operation_noop_result(operator);
                };
                self.buffer_view.set_cursor(new_cursor);
                ActionResult::Handled
            }
            Some(ModeKind::Visual) => {
                let Some(range) = self.buffer_view.visual_selection_range() else {
                    return self.operation_noop_result(operator);
                };
                let Some(text) = self.capture_characterwise_text(range.start, range.end) else {
                    return self.operation_noop_result(operator);
                };
                let transformed = Self::transform_case_text(&text, operator);
                let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| {
                        let cursor = buffer.delete_range(range)?;
                        buffer.insert_text(range.start, &transformed);
                        Some(cursor)
                    })
                    .flatten()
                else {
                    return self.operation_noop_result(operator);
                };
                self.buffer_view.set_cursor(new_cursor);
                ActionResult::Handled
            }
            _ => self.operation_noop_result(operator),
        }
    }

    fn handle_operation_with_count(
        &mut self,
        operator: Operator,
        target: OperatorTarget,
        count: usize,
        from_mode: Option<ModeKind>,
        register: Option<RegisterName>,
    ) -> ActionResult {
        match target {
            OperatorTarget::Selection => self.handle_case_selection(operator, from_mode),
            OperatorTarget::LinewiseMotion(motion) => {
                self.handle_linewise_operation_with_count(operator, motion, count, register)
            }
            _ => self.handle_characterwise_operation_with_count(operator, target, count, register),
        }
    }

    fn handle_characterwise_operation_with_count(
        &mut self,
        operator: Operator,
        target: OperatorTarget,
        count: usize,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let range = self
            .buffer_view
            .with_buffer(|buffer| {
                buffer.get_operator_target_range_with_count(cursor, target, count)
            })
            .flatten();
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };

        let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_range(range.start, range.end))
            .flatten()
        else {
            return self.operation_noop_result(operator);
        };

        match operator {
            Operator::Delete => self.store_register_content(
                register,
                DefaultRegisterRole::Delete,
                text,
                RegisterContentKind::Characterwise,
            ),
            Operator::Change => self.store_register_content(
                register,
                DefaultRegisterRole::Change,
                text,
                RegisterContentKind::Characterwise,
            ),
            Operator::Yank => {
                self.store_register_content(
                    register,
                    DefaultRegisterRole::Yank,
                    text,
                    RegisterContentKind::Characterwise,
                );
                return ActionResult::Handled;
            }
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                let transformed = Self::transform_case_text(&text, operator);
                if range.start == range.end {
                    return ActionResult::Handled;
                }
                let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| {
                        let cursor = buffer.delete_range(range)?;
                        buffer.insert_text(range.start, &transformed);
                        Some(cursor)
                    })
                    .flatten()
                else {
                    return self.operation_noop_result(operator);
                };
                self.buffer_view.set_cursor(new_cursor);
                return ActionResult::Handled;
            }
        }

        if range.start == range.end {
            if matches!(operator, Operator::Change) {
                self.buffer_view.set_cursor(range.start);
            }
            return ActionResult::Handled;
        }

        let result = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.delete_range(range));
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
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let range = self
            .buffer_view
            .with_buffer(|buffer| {
                buffer.get_linewise_operator_target_range_with_count(cursor, motion, count)
            })
            .flatten();
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };

        let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(range.start_line, range.count))
            .flatten()
        else {
            return self.operation_noop_result(operator);
        };

        match operator {
            Operator::Delete => self.store_register_content(
                register,
                DefaultRegisterRole::Delete,
                text,
                RegisterContentKind::Linewise,
            ),
            Operator::Change => self.store_register_content(
                register,
                DefaultRegisterRole::Change,
                text,
                RegisterContentKind::Linewise,
            ),
            Operator::Yank => {
                self.store_register_content(
                    register,
                    DefaultRegisterRole::Yank,
                    text,
                    RegisterContentKind::Linewise,
                );
                return ActionResult::Handled;
            }
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                let transformed = Self::transform_case_text(&text, operator);
                if range.count == 0 {
                    return ActionResult::Handled;
                }
                let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| {
                        let cursor = buffer.delete_lines(range.start_line, range.count)?;
                        buffer.insert_text(Cursor::new(range.start_line, 0), &transformed);
                        Some(cursor)
                    })
                    .flatten()
                else {
                    return self.operation_noop_result(operator);
                };
                self.buffer_view.set_cursor(new_cursor);
                return ActionResult::Handled;
            }
        }

        if range.count == 0 {
            return ActionResult::Handled;
        }

        let result = self.buffer_view.with_buffer_mut(|buffer| match operator {
            Operator::Delete | Operator::Change => {
                buffer.delete_lines(range.start_line, range.count)
            }
            Operator::Yank | Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                None
            }
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
        from_mode: Option<ModeKind>,
        register: Option<RegisterName>,
    ) -> ActionResult {
        match target {
            OperatorTarget::Selection => self.handle_case_selection(*operator, from_mode),
            OperatorTarget::LinewiseMotion(motion) => {
                self.handle_linewise_operation(*operator, *motion, register)
            }
            _ => self.handle_characterwise_operation_with_count(*operator, *target, 1, register),
        }
    }

    fn handle_linewise_operation(
        &mut self,
        operator: Operator,
        motion: LinewiseMotion,
        register: Option<RegisterName>,
    ) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        let range = self
            .buffer_view
            .with_buffer(|buffer| buffer.get_linewise_operator_target_range(cursor, motion))
            .flatten();
        let Some(range) = range else {
            return self.operation_noop_result(operator);
        };

        let Some(text) = self
            .buffer_view
            .with_buffer(|buffer| buffer.text_in_lines(range.start_line, range.count))
            .flatten()
        else {
            return self.operation_noop_result(operator);
        };

        match operator {
            Operator::Delete => self.store_register_content(
                register,
                DefaultRegisterRole::Delete,
                text,
                RegisterContentKind::Linewise,
            ),
            Operator::Change => self.store_register_content(
                register,
                DefaultRegisterRole::Change,
                text,
                RegisterContentKind::Linewise,
            ),
            Operator::Yank => {
                self.store_register_content(
                    register,
                    DefaultRegisterRole::Yank,
                    text,
                    RegisterContentKind::Linewise,
                );
                return ActionResult::Handled;
            }
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                let transformed = Self::transform_case_text(&text, operator);
                if range.count == 0 {
                    return ActionResult::Handled;
                }
                let Some(new_cursor) = self
                    .buffer_view
                    .with_buffer_mut(|buffer| {
                        let cursor = buffer.delete_lines(range.start_line, range.count)?;
                        buffer.insert_text(Cursor::new(range.start_line, 0), &transformed);
                        Some(cursor)
                    })
                    .flatten()
                else {
                    return self.operation_noop_result(operator);
                };
                self.buffer_view.set_cursor(new_cursor);
                return ActionResult::Handled;
            }
        }

        if range.count == 0 {
            return ActionResult::Handled;
        }

        let result = self.buffer_view.with_buffer_mut(|buffer| match operator {
            Operator::Delete | Operator::Change => {
                buffer.delete_lines(range.start_line, range.count)
            }
            Operator::Yank | Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                None
            }
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
            Operator::Yank | Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase => {
                ActionResult::Handled
            }
        }
    }
}
