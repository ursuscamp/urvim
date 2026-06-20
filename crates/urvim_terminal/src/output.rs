use super::*;

#[allow(dead_code)]
impl<I: Read + AsFd, O: Write + AsFd> Terminal<I, O> {
    pub fn flush(&mut self) -> io::Result<()> {
        if self.flush {
            self.output.flush()?;
        }
        Ok(())
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.output.write_all(CLEAR_SCREEN.as_bytes())?;
        self.flush()
    }

    pub fn set_flush(&mut self, enabled: bool) {
        self.flush = enabled;
    }

    pub fn batch<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Terminal<I, O>) -> io::Result<()>,
    {
        let prev_flush = self.flush;
        self.flush = false;
        f(self)?;
        self.flush = prev_flush;
        self.output.flush()
    }

    pub fn set_cursor_position(&mut self, row: u16, col: u16) -> io::Result<()> {
        let mut buf = [0u8; 16];
        let mut i = 0;
        buf[i] = b'\x1b';
        i += 1;
        buf[i] = b'[';
        i += 1;
        i = write_decimal(row, &mut buf, i);
        buf[i] = b';';
        i += 1;
        i = write_decimal(col, &mut buf, i);
        buf[i] = b'H';
        i += 1;
        self.output.write_all(&buf[..i])?;
        self.flush()?;
        Ok(())
    }

    pub fn get_cursor_position(&mut self) -> io::Result<(u16, u16)> {
        self.output.write_all(b"\x1b[6n")?;
        self.flush()?;
        query_cursor_position(&mut self.input, &mut self.output, false)
            .ok_or_else(|| io::Error::other("failed to get cursor position"))
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.output.write_all(SHOW_CURSOR.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.output.write_all(HIDE_CURSOR.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    pub fn set_cursor_style(&mut self, style: CursorStyle) -> io::Result<()> {
        self.output.write_all(style.as_str().as_bytes())?;
        self.flush()?;
        Ok(())
    }

    pub fn set_style(&mut self, style: &Style) -> io::Result<()> {
        style.write_escape_code(&mut self.output)?;
        self.flush()?;
        Ok(())
    }

    pub fn reset_style(&mut self) -> io::Result<()> {
        self.output.write_all(b"\x1b[0m")?;
        self.flush()?;
        Ok(())
    }

    pub fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.output.write_all(text.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    pub fn copy_to_clipboard(&mut self, text: &str) -> io::Result<()> {
        let seq = osc52_copy_to_clipboard(text);
        self.output.write_all(&seq)?;
        self.flush()?;
        Ok(())
    }

    pub fn detect_text_sizing_support(&mut self) -> io::Result<TextSizingSupport> {
        use sizing::TextSizingSupport::*;

        let old_pos = self.get_cursor_position()?;
        let pos1 = self.get_cursor_position()?;
        self.output.write_all(b"\x1b]66;w=2; \x07")?;
        self.flush()?;
        let pos2 = self.get_cursor_position()?;
        self.output.write_all(b"\x1b]66;s=2; \x07")?;
        self.flush()?;
        let pos3 = self.get_cursor_position()?;
        self.set_cursor_position(old_pos.0, old_pos.1)?;

        if pos2.1 == pos1.1 && pos3.1 == pos1.1 {
            return Ok(None);
        }
        if pos3.1 > pos2.1 {
            return Ok(Full);
        }
        Ok(WidthOnly)
    }

    pub fn write_styled_text<S: AsRef<str>>(
        &mut self,
        style: Option<&Style>,
        sizing: Option<&TextSizing>,
        text: S,
    ) -> io::Result<()> {
        if let Some(s) = style {
            s.write_escape_code(&mut self.output)?;
        }
        if let Some(s) = sizing {
            s.write_escape_code(&mut self.output)?;
        }
        self.output.write_all(text.as_ref().as_bytes())?;
        if sizing.is_some() {
            self.output.write_all(b"\x07")?;
        }
        self.flush()?;
        Ok(())
    }
}
