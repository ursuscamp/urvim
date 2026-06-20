use super::*;

impl Buffer {
    fn advance_while(
        line: &impl TextRef,
        mut col: usize,
        predicate: impl Fn(&str) -> bool,
    ) -> usize {
        while col < line.len() {
            let Some(grapheme) = line.next_grapheme(col) else {
                break;
            };
            if grapheme.byte_idx() != col || !predicate(grapheme.as_str()) {
                break;
            }
            col += grapheme.len();
        }
        col
    }

    fn grapheme_at_matches(
        line: &impl TextRef,
        col: usize,
        predicate: impl Fn(&str) -> bool,
    ) -> bool {
        line.next_grapheme(col)
            .is_some_and(|grapheme| grapheme.byte_idx() == col && predicate(grapheme.as_str()))
    }

    /// Check if a grapheme is a word character (alphanumeric or underscore).
    pub fn is_word_char(grapheme: &str) -> bool {
        let mut chars = grapheme.chars();
        match chars.next() {
            Some(c) => c.is_alphanumeric() || c == '_',
            None => false,
        }
    }

    /// Check if a grapheme is a whitespace character.
    pub fn is_whitespace_char(grapheme: &str) -> bool {
        let mut chars = grapheme.chars();
        match chars.next() {
            Some(c) => c.is_whitespace(),
            None => false,
        }
    }

    /// Check if a grapheme is a BigWord character (non-whitespace).
    pub fn is_bigword_char(grapheme: &str) -> bool {
        !Self::is_whitespace_char(grapheme)
    }

    /// Check if cursor is at the specified boundary.
    pub fn is_at_boundary(&self, cursor: Cursor, boundary: Boundary) -> bool {
        let line_idx = cursor.line;
        let col = cursor.col;

        let current_grapheme = self.grapheme_at_byte(line_idx, col);
        let prev_grapheme = self.prev_grapheme_before_byte(line_idx, col);
        let next_grapheme = self.next_grapheme_at_or_after_byte(line_idx, col);

        match boundary {
            Boundary::Word => match current_grapheme.as_deref() {
                Some(g) if Self::is_word_char(g) => match prev_grapheme.as_deref() {
                    Some(pg) => !Self::is_word_char(pg),
                    None => true,
                },
                Some(g) if !Self::is_word_char(g) && !Self::is_whitespace_char(g) => {
                    match prev_grapheme.as_deref() {
                        Some(pg) => Self::is_word_char(pg),
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::WordEnd => match prev_grapheme.as_deref() {
                Some(pg) if Self::is_word_char(pg) => match next_grapheme.as_deref() {
                    Some(ng) => !Self::is_word_char(ng),
                    None => true,
                },
                Some(pg) if !Self::is_word_char(pg) && !Self::is_whitespace_char(pg) => {
                    match next_grapheme.as_deref() {
                        Some(ng) => {
                            Self::is_word_char(ng)
                                || (!Self::is_word_char(ng) && !Self::is_whitespace_char(ng))
                        }
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::BigWord => match current_grapheme.as_deref() {
                Some(g) if Self::is_bigword_char(g) => match prev_grapheme.as_deref() {
                    Some(pg) => Self::is_whitespace_char(pg),
                    None => true,
                },
                _ => false,
            },
            Boundary::BigWordEnd => match prev_grapheme.as_deref() {
                Some(pg) if Self::is_bigword_char(pg) => match next_grapheme.as_deref() {
                    Some(ng) => Self::is_whitespace_char(ng),
                    None => true,
                },
                _ => false,
            },
        }
    }

    /// Find the next boundary position forward from cursor.
    ///
    /// Returns None if no boundary exists in the forward direction.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim_core::buffer::{Buffer, Cursor, Boundary};
    ///
    /// let buf = Buffer::from_str("hello world");
    /// let next = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    /// assert_eq!(next, Some(Cursor::new(0, 6))); // at 'w'
    /// ```
    pub fn next_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor> {
        let total_lines = self.line_count();
        let mut line_idx = cursor.line;
        let mut col = cursor.col;

        // If at end of line, move to next line
        if col >= self.line_len(line_idx) {
            line_idx += 1;
            col = 0;
        }

        while line_idx < total_lines {
            let line = match self.line_at(line_idx) {
                Some(l) => l,
                None => break,
            };

            let line_len = line.len();

            // Skip empty lines
            if line_len == 0 {
                line_idx += 1;
                col = 0;
                continue;
            }

            // Clamp col
            if col >= line_len {
                // Wrapping to new line - first check if we're at start of a word
                // (this handles the case where a line starts with a word without leading whitespace)
                if col == 0 && line_len > 0 {
                    if Self::grapheme_at_matches(&line, 0, Self::is_word_char) {
                        return Some(Cursor::new(line_idx, 0));
                    }
                }
                line_idx += 1;
                col = 0;
                continue;
            }

            match boundary {
                Boundary::Word => {
                    // Skip to end of current word, then find next word start
                    let mut check_col = col;

                    // Check if we started at a word character
                    let started_at_word_char =
                        col < line_len && Self::grapheme_at_matches(&line, col, Self::is_word_char);

                    // If we're at a word char, skip to end of it
                    check_col = Self::advance_while(&line, check_col, Self::is_word_char);

                    // Now we're past the current word (or at the end of line)
                    // Check if the next character is a non-word, non-whitespace character (e.g., "---")
                    // If we came FROM a word, this is a boundary - return the position
                    // If we started at a non-word, skip through and find the next boundary
                    if check_col < line_len {
                        if let Some(gg) = line.next_grapheme(check_col)
                            && gg.byte_idx() == check_col
                            && !Self::is_word_char(gg.as_str())
                            && !Self::is_whitespace_char(gg.as_str())
                        {
                            if started_at_word_char {
                                // We came from a word - this non-word sequence is a separate word
                                // Return the start of it
                                return Some(Cursor::new(line_idx, check_col));
                            } else {
                                // We started at a non-word - skip through the sequence
                                check_col = Self::advance_while(&line, check_col, |grapheme| {
                                    !Self::is_word_char(grapheme)
                                        && !Self::is_whitespace_char(grapheme)
                                });
                            }
                        }
                    }

                    // Skip whitespace to find the next word-like run. Punctuation
                    // separated from a word by spaces is still its own `w` target.
                    while check_col < line_len {
                        match line.next_grapheme(check_col) {
                            Some(gg)
                                if gg.byte_idx() == check_col
                                    && Self::is_word_char(gg.as_str()) =>
                            {
                                // Found start of next word - return this position
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg)
                                if gg.byte_idx() == check_col
                                    && !Self::is_whitespace_char(gg.as_str()) =>
                            {
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) if gg.byte_idx() == check_col => {
                                check_col += gg.len();
                            }
                            None => break,
                            _ => break,
                        }
                    }

                    // No more words on this line - wrap to next line
                    // When wrapping, check if next line starts with a word (without leading whitespace)
                    line_idx += 1;
                    col = 0;
                    // Check if the new line starts with a word character
                    if line_idx < total_lines {
                        let next_line = self.line_at(line_idx);
                        if let Some(l) = next_line {
                            if !l.is_empty() && Self::grapheme_at_matches(&l, 0, Self::is_word_char)
                            {
                                return Some(Cursor::new(line_idx, 0));
                            }
                        }
                    }
                    continue;
                }

                Boundary::WordEnd => {
                    // If we're on a word character, go to the end of THIS word
                    // If we're at the last word of the line, wrap to next line
                    // Otherwise, find the end of the next word
                    let mut check_col = col;

                    // Check if we're on a word character
                    let on_word_char = check_col < line_len
                        && Self::grapheme_at_matches(&line, check_col, Self::is_word_char);

                    // Check if we're on a non-word, non-whitespace character (e.g., "---")
                    let on_non_word_non_ws = check_col < line_len
                        && Self::grapheme_at_matches(&line, check_col, |grapheme| {
                            !Self::is_word_char(grapheme) && !Self::is_whitespace_char(grapheme)
                        });

                    if on_non_word_non_ws {
                        // We're at a non-word, non-whitespace char - find its end
                        // This is the end of this "word" (the non-word chars)
                        check_col = Self::advance_while(&line, check_col, |grapheme| {
                            !Self::is_word_char(grapheme) && !Self::is_whitespace_char(grapheme)
                        });

                        // If we're at the end of the non-word sequence and it's different from where we started,
                        // return the end of this "word"
                        if check_col > col {
                            // Check if we're at a new position or still at the same position
                            if check_col - 1 > col {
                                return Some(Cursor::new(line_idx, check_col - 1));
                            }
                            // We're at the end of a non-word sequence but were already at its last char
                            // Continue to find the next word end
                        }

                        // Skip any whitespace and find the next word
                        check_col = Self::advance_while(&line, check_col, |grapheme| {
                            !Self::is_word_char(grapheme)
                        });

                        // Now find the end of that word
                        let end_col = Self::advance_while(&line, check_col, Self::is_word_char);
                        if end_col > check_col {
                            return Some(Cursor::new(line_idx, end_col - 1));
                        }
                    } else if on_word_char && check_col < line_len {
                        // We're in a word - find its end
                        let mut at_end_of_line = false;
                        while check_col < line_len {
                            match line.next_grapheme(check_col) {
                                Some(gg)
                                    if gg.byte_idx() == check_col
                                        && Self::is_word_char(gg.as_str()) =>
                                {
                                    // Check if this is the last char of the line
                                    let next_check = check_col + gg.len();
                                    if next_check >= line_len {
                                        at_end_of_line = true;
                                    }
                                    check_col = next_check;
                                }
                                _ => break,
                            }
                        }

                        // If we're NOT at end of line, check if we actually moved forward
                        // past the current word. If we're still at the same position
                        // (meaning we were already at a word end), skip to next word.
                        if !at_end_of_line && check_col > col + 1 {
                            // We moved past at least one character - return end of current word
                            return Some(Cursor::new(line_idx, check_col - 1));
                        } else if !at_end_of_line && check_col == col + 1 {
                            // We were at a word end position - skip whitespace and find next word end
                            // But first, check if we're at a non-word, non-whitespace sequence
                            // If so, that's the end of the next "word" - return it
                            if check_col < line_len {
                                if let Some(gg) = line.next_grapheme(check_col)
                                    && gg.byte_idx() == check_col
                                    && !Self::is_word_char(gg.as_str())
                                    && !Self::is_whitespace_char(gg.as_str())
                                {
                                    // We're at a non-word, non-whitespace sequence
                                    // Find its end and return
                                    let end_col =
                                        Self::advance_while(&line, check_col, |grapheme| {
                                            !Self::is_word_char(grapheme)
                                                && !Self::is_whitespace_char(grapheme)
                                        });
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                            }

                            // Find next word start (skip whitespace only)
                            check_col = Self::advance_while(&line, check_col, |grapheme| {
                                Self::is_whitespace_char(grapheme)
                            });
                            // Now find the end of that next word
                            let end_col = Self::advance_while(&line, check_col, Self::is_word_char);
                            if end_col > check_col {
                                return Some(Cursor::new(line_idx, end_col - 1));
                            }
                        }
                    }

                    // Either not on a word, or at end of line - wrap to next line
                    // Check if next line starts with a word and find its end
                    line_idx += 1;
                    let _col = 0;

                    // Find word on next line and return its end
                    while line_idx < total_lines {
                        let next_line = match self.line_at(line_idx) {
                            Some(l) => l,
                            None => break,
                        };
                        let next_line_len = next_line.len();

                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Find start of word on this line
                        let mut check_col = 0;
                        while check_col < next_line_len {
                            match next_line.next_grapheme(check_col) {
                                Some(gg)
                                    if gg.byte_idx() == check_col
                                        && Self::is_word_char(gg.as_str()) =>
                                {
                                    // Found word start - find its end
                                    let end_col = Self::advance_while(
                                        &next_line,
                                        check_col,
                                        Self::is_word_char,
                                    );
                                    // Return position at end of word (not after)
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                                Some(gg) if gg.byte_idx() == check_col => {
                                    check_col += gg.len();
                                }
                                None => break,
                                _ => break,
                            }
                        }

                        // No word found on this line, continue to next
                        line_idx += 1;
                    }
                }
                Boundary::BigWord => {
                    // First, skip to end of current bigword if we're in one
                    let mut check_col = col;
                    check_col = Self::advance_while(&line, check_col, Self::is_bigword_char);
                    // Now skip whitespace to find next bigword
                    while check_col < line_len {
                        match line.next_grapheme(check_col) {
                            Some(gg)
                                if gg.byte_idx() == check_col
                                    && Self::is_bigword_char(gg.as_str()) =>
                            {
                                // Found next bigword start
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) if gg.byte_idx() == check_col => {
                                check_col += gg.len();
                            }
                            None => break,
                            _ => break,
                        }
                    }

                    // No more bigwords on this line - wrap to next line
                    // When wrapping, check if next line starts with a bigword (non-whitespace)
                    // If it starts with whitespace, skip it and find the first bigword
                    line_idx += 1;
                    let _col = 0;

                    while line_idx < total_lines {
                        let next_line = match self.line_at(line_idx) {
                            Some(l) => l,
                            None => break,
                        };
                        let next_line_len = next_line.len();
                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Check if first char is a bigword char (non-whitespace)
                        if Self::grapheme_at_matches(&next_line, 0, Self::is_bigword_char) {
                            // Line starts with a bigword - return position 0
                            return Some(Cursor::new(line_idx, 0));
                        } else {
                            // Line starts with whitespace - skip it and find first bigword
                            let mut check_col = 0;
                            while check_col < next_line_len {
                                match next_line.next_grapheme(check_col) {
                                    Some(gg)
                                        if gg.byte_idx() == check_col
                                            && Self::is_bigword_char(gg.as_str()) =>
                                    {
                                        // Found first bigword on this line
                                        return Some(Cursor::new(line_idx, check_col));
                                    }
                                    Some(gg) if gg.byte_idx() == check_col => {
                                        check_col += gg.len();
                                    }
                                    None => break,
                                    _ => break,
                                }
                            }
                            // No bigword found on this line, continue to next
                            line_idx += 1;
                        }
                    }
                }
                Boundary::BigWordEnd => {
                    // Find end of current bigword, then find end of next bigword
                    let mut check_col = col;
                    let start_col = col;

                    // First, skip to end of current bigword if we're in one
                    check_col = Self::advance_while(&line, check_col, Self::is_bigword_char);

                    // After first while, check_col is at end of current word or past it
                    // If we moved forward past the starting position, check what comes after
                    if check_col > start_col {
                        let after_current = if check_col < line_len {
                            line.next_grapheme(check_col)
                        } else {
                            None
                        };

                        match after_current {
                            Some(gg)
                                if gg.byte_idx() == check_col
                                    && Self::is_bigword_char(gg.as_str()) =>
                            {
                                // Another word right after - continue to find it
                            }
                            Some(gg)
                                if gg.byte_idx() == check_col
                                    && Self::is_whitespace_char(gg.as_str()) =>
                            {
                                // Whitespace after - if we moved to a NEW position (not same as start),
                                // return end of current word. But if we're at same position as start
                                // (e.g., single char), find next word instead.
                                if check_col - 1 > start_col {
                                    return Some(Cursor::new(line_idx, check_col - 1));
                                }
                                // Fall through to find next word
                            }
                            None => {
                                // End of line - don't return here, fall through to wrap
                            }
                            _ => {}
                        }
                    }

                    // Try to find next word on current line (skip whitespace, find word)
                    // Track original position to know if we found whitespace
                    let pre_whitespace_col = check_col;

                    // Skip whitespace to find next bigword
                    check_col = Self::advance_while(&line, check_col, |grapheme| {
                        !Self::is_bigword_char(grapheme)
                    });

                    // Now at start of next bigword, find its end
                    check_col = Self::advance_while(&line, check_col, Self::is_bigword_char);

                    // Return position AT last character (not after)
                    // Only return if we found a next word (check_col advanced past pre_whitespace_col)
                    // AND we moved forward from start
                    let found_next_word = check_col > pre_whitespace_col && check_col > start_col;
                    // Special case: if we started at position 0 (wrapped from previous line) and found a word
                    let started_at_zero = start_col == 0 && check_col > 0;

                    if (found_next_word || started_at_zero) && check_col <= line_len + 1 {
                        return Some(Cursor::new(line_idx, check_col - 1));
                    }
                    // No next word found on this line - fall through to wrap to next line
                }
            }

            // Move to next line
            line_idx += 1;
            col = 0;
        }

        None
    }

    /// Find the previous boundary position backward from cursor.
    ///
    /// Returns None if no boundary exists in the backward direction.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim_core::buffer::{Buffer, Cursor, Boundary};
    ///
    /// let buf = Buffer::from_str("hello world");
    /// let prev = buf.prev_boundary(Cursor::new(0, 6), Boundary::Word);
    /// assert_eq!(prev, Some(Cursor::new(0, 0))); // at 'h'
    /// ```
    pub fn prev_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor> {
        let mut line_idx = cursor.line;
        let mut col = cursor.col;

        // If at start of line, move to end of previous line
        if col == 0 {
            if line_idx == 0 {
                return None;
            }
            line_idx -= 1;
            col = self.line_len(line_idx);
        }

        loop {
            if line_idx >= self.line_count() {
                if line_idx == 0 {
                    return None;
                }
                line_idx -= 1;
                col = self.line_len(line_idx);
                continue;
            }

            let line = self.line_at(line_idx)?;

            let line_len = line.len();

            if line_len == 0 {
                if line_idx == 0 {
                    return None;
                }
                line_idx -= 1;
                col = self.line_len(line_idx);
                continue;
            }

            // Clamp col
            if col > line_len {
                col = line_len;
            }

            // Scan backward looking for boundary
            let mut check_col = col;
            while check_col > 0 {
                // Move back one grapheme
                let Some(prev_grapheme) = line.previous_grapheme(check_col) else {
                    break;
                };
                check_col = prev_grapheme.byte_idx();

                // Check if this position is a boundary (not the starting position)
                if check_col < col {
                    let check_cursor = Cursor::new(line_idx, check_col);
                    if self.is_at_boundary(check_cursor, boundary) {
                        return Some(check_cursor);
                    }
                }
            }

            // Try previous line
            if line_idx == 0 {
                return None;
            }
            line_idx -= 1;
            col = self.line_len(line_idx);
        }
    }
}
