use super::*;

impl Buffer {
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
            Boundary::Word => match current_grapheme {
                Some(g) if Self::is_word_char(g) => match prev_grapheme {
                    Some(pg) => !Self::is_word_char(pg),
                    None => true,
                },
                Some(g) if !Self::is_word_char(g) && !Self::is_whitespace_char(g) => {
                    match prev_grapheme {
                        Some(pg) => Self::is_word_char(pg),
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::WordEnd => match prev_grapheme {
                Some(pg) if Self::is_word_char(pg) => match next_grapheme {
                    Some(ng) => !Self::is_word_char(ng),
                    None => true,
                },
                Some(pg) if !Self::is_word_char(pg) && !Self::is_whitespace_char(pg) => {
                    match next_grapheme {
                        Some(ng) => {
                            Self::is_word_char(ng)
                                || (!Self::is_word_char(ng) && !Self::is_whitespace_char(ng))
                        }
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::BigWord => match current_grapheme {
                Some(g) if Self::is_bigword_char(g) => match prev_grapheme {
                    Some(pg) => Self::is_whitespace_char(pg),
                    None => true,
                },
                _ => false,
            },
            Boundary::BigWordEnd => match prev_grapheme {
                Some(pg) if Self::is_bigword_char(pg) => match next_grapheme {
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
    /// use urvim::buffer::{Buffer, Cursor, Boundary};
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

            let line_str = line.as_ref();
            let line_len = line_str.len();

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
                    let g = line_str.get(0..).and_then(|s| s.graphemes(true).next());
                    if matches!(g, Some(gg) if Self::is_word_char(gg)) {
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
                    let started_at_word_char = if col < line_len {
                        let g = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if Self::is_word_char(gg))
                    } else {
                        false
                    };

                    // If we're at a word char, skip to end of it
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_word_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

                    // Now we're past the current word (or at the end of line)
                    // Check if the next character is a non-word, non-whitespace character (e.g., "---")
                    // If we came FROM a word, this is a boundary - return the position
                    // If we started at a non-word, skip through and find the next boundary
                    if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        if let Some(gg) = g
                            && !Self::is_word_char(gg)
                            && !Self::is_whitespace_char(gg)
                        {
                            if started_at_word_char {
                                // We came from a word - this non-word sequence is a separate word
                                // Return the start of it
                                return Some(Cursor::new(line_idx, check_col));
                            } else {
                                // We started at a non-word - skip through the sequence
                                while check_col < line_len {
                                    let g = line_str
                                        .get(check_col..)
                                        .and_then(|s| s.graphemes(true).next());
                                    match g {
                                        Some(gg)
                                            if !Self::is_word_char(gg)
                                                && !Self::is_whitespace_char(gg) =>
                                        {
                                            check_col += gg.len();
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                    }

                    // Skip whitespace to find the next word
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_word_char(gg) => {
                                // Found start of next word - return this position
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) => {
                                check_col += gg.len();
                            }
                            None => break,
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
                            let next_line_str = l.as_ref();
                            if !next_line_str.is_empty() {
                                let first_g = next_line_str.graphemes(true).next();
                                if matches!(first_g, Some(g) if Self::is_word_char(g)) {
                                    return Some(Cursor::new(line_idx, 0));
                                }
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
                    let on_word_char = if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if Self::is_word_char(gg))
                    } else {
                        false
                    };

                    // Check if we're on a non-word, non-whitespace character (e.g., "---")
                    let on_non_word_non_ws = if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if !Self::is_word_char(gg) && !Self::is_whitespace_char(gg))
                    } else {
                        false
                    };

                    if on_non_word_non_ws {
                        // We're at a non-word, non-whitespace char - find its end
                        // This is the end of this "word" (the non-word chars)
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg)
                                    if !Self::is_word_char(gg) && !Self::is_whitespace_char(gg) =>
                                {
                                    check_col += gg.len();
                                }
                                _ => break,
                            }
                        }

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
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => break,
                                Some(gg) if Self::is_whitespace_char(gg) => check_col += gg.len(),
                                Some(gg) => check_col += gg.len(),
                                None => break,
                            }
                        }

                        // Now find the end of that word
                        let mut end_col = check_col;
                        while end_col < line_len {
                            let g = line_str
                                .get(end_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
                                    end_col += gg.len();
                                }
                                _ => break,
                            }
                        }
                        if end_col > check_col {
                            return Some(Cursor::new(line_idx, end_col - 1));
                        }
                    } else if on_word_char && check_col < line_len {
                        // We're in a word - find its end
                        let mut at_end_of_line = false;
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
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
                                let g = line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                if let Some(gg) = g
                                    && !Self::is_word_char(gg)
                                    && !Self::is_whitespace_char(gg)
                                {
                                    // We're at a non-word, non-whitespace sequence
                                    // Find its end and return
                                    let mut end_col = check_col;
                                    while end_col < line_len {
                                        let g = line_str
                                            .get(end_col..)
                                            .and_then(|s| s.graphemes(true).next());
                                        match g {
                                            Some(gg)
                                                if !Self::is_word_char(gg)
                                                    && !Self::is_whitespace_char(gg) =>
                                            {
                                                end_col += gg.len();
                                            }
                                            _ => break,
                                        }
                                    }
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                            }

                            // Find next word start (skip whitespace only)
                            while check_col < line_len {
                                let g = line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_word_char(gg) => break,
                                    Some(gg) if Self::is_whitespace_char(gg) => {
                                        check_col += gg.len()
                                    }
                                    Some(gg) => {
                                        // Hit a non-word, non-whitespace - we've already handled this above
                                        // This shouldn't be reached
                                        let _ = gg; // suppress unused warning
                                        break;
                                    }
                                    None => break,
                                }
                            }
                            // Now find the end of that next word
                            let mut end_col = check_col;
                            while end_col < line_len {
                                let g = line_str
                                    .get(end_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_word_char(gg) => {
                                        end_col += gg.len();
                                    }
                                    _ => break,
                                }
                            }
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
                        let next_line_str = next_line.as_ref();
                        let next_line_len = next_line_str.len();

                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Find start of word on this line
                        let mut check_col = 0;
                        while check_col < next_line_len {
                            let g = next_line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
                                    // Found word start - find its end
                                    let mut end_col = check_col;
                                    while end_col < next_line_len {
                                        let gg = next_line_str
                                            .get(end_col..)
                                            .and_then(|s| s.graphemes(true).next());
                                        match gg {
                                            Some(gc) if Self::is_word_char(gc) => {
                                                end_col += gc.len();
                                            }
                                            _ => break,
                                        }
                                    }
                                    // Return position at end of word (not after)
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                                Some(gg) => {
                                    check_col += gg.len();
                                }
                                None => break,
                            }
                        }

                        // No word found on this line, continue to next
                        line_idx += 1;
                    }
                }
                Boundary::BigWord => {
                    // First, skip to end of current bigword if we're in one
                    let mut check_col = col;
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }
                    // Now skip whitespace to find next bigword
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                // Found next bigword start
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) => {
                                check_col += gg.len();
                            }
                            None => break,
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
                        let next_line_str = next_line.as_ref();
                        let next_line_len = next_line_str.len();
                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Check if first char is a bigword char (non-whitespace)
                        let first_g = next_line_str.graphemes(true).next();
                        if matches!(first_g, Some(g) if Self::is_bigword_char(g)) {
                            // Line starts with a bigword - return position 0
                            return Some(Cursor::new(line_idx, 0));
                        } else {
                            // Line starts with whitespace - skip it and find first bigword
                            let mut check_col = 0;
                            while check_col < next_line_len {
                                let g = next_line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_bigword_char(gg) => {
                                        // Found first bigword on this line
                                        return Some(Cursor::new(line_idx, check_col));
                                    }
                                    Some(gg) => {
                                        check_col += gg.len();
                                    }
                                    None => break,
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
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

                    // After first while, check_col is at end of current word or past it
                    // If we moved forward past the starting position, check what comes after
                    if check_col > start_col {
                        let after_current = if check_col < line_len {
                            line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next())
                        } else {
                            None
                        };

                        match after_current {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                // Another word right after - continue to find it
                            }
                            Some(gg) if Self::is_whitespace_char(gg) => {
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
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => break,
                            Some(gg) => check_col += gg.len(),
                            None => break,
                        }
                    }

                    // Now at start of next bigword, find its end
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

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
    /// use urvim::buffer::{Buffer, Cursor, Boundary};
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

            let line_str = line.as_ref();
            let line_len = line_str.len();

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
                let mut prev_offset = 0;
                let mut found = false;
                for (byte_offset, _g) in line_str.grapheme_indices(true) {
                    if byte_offset >= check_col {
                        break;
                    }
                    prev_offset = byte_offset;
                    found = true;
                }
                if !found {
                    break;
                }
                check_col = prev_offset;

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
