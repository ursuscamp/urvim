use super::*;

#[allow(dead_code)]
impl<I: Read + AsFd, O: Write + AsFd> Terminal<I, O> {
    /// Creates a new terminal instance.
    pub fn new(input: I, output: O) -> io::Result<Self> {
        let original = tcgetattr(&input).map_err(io::Error::from)?;
        let mut termios = original.clone();
        termios.make_raw();
        tcsetattr(&input, OptionalActions::Now, &termios).map_err(io::Error::from)?;

        let mut output = output;
        output.write_all(ENTER_ALTERNATIVE_SCREEN.as_bytes())?;
        output.write_all(CLEAR_SCREEN.as_bytes())?;
        output.write_all(ENABLE_CSI_U.as_bytes())?;
        output.write_all(ENABLE_BRACKETED_PASTE.as_bytes())?;
        output.flush()?;

        let (rows, cols) = get_terminal_size().unwrap_or((24, 80));
        let is_tty = is_terminal::is_terminal(std::io::stdin());

        Ok(Self {
            input,
            output,
            original: Some(original),
            buffer: ByteBuffer::new(),
            paste_active: false,
            last_rows: rows,
            last_cols: cols,
            is_tty,
            flush: true,
        })
    }

    /// Creates a new terminal instance for testing without TTY features.
    pub fn new_for_testing(input: I, output: O) -> Self {
        Self {
            input,
            output,
            original: None,
            buffer: ByteBuffer::new(),
            paste_active: false,
            last_rows: 24,
            last_cols: 80,
            is_tty: false,
            flush: true,
        }
    }

    /// Creates a new terminal instance for testing with an explicit TTY flag.
    #[cfg(test)]
    pub fn new_for_testing_with_tty(input: I, output: O, is_tty: bool) -> Self {
        Self {
            input,
            output,
            original: None,
            buffer: ByteBuffer::new(),
            paste_active: false,
            last_rows: 24,
            last_cols: 80,
            is_tty,
            flush: true,
        }
    }

    /// Restores the terminal to its original state.
    pub fn restore(&mut self) -> io::Result<()> {
        self.output.write_all(DISABLE_CSI_U.as_bytes())?;
        self.output.flush()?;
        self.output.write_all(DISABLE_BRACKETED_PASTE.as_bytes())?;
        self.output.flush()?;
        self.output.write_all(EXIT_ALTERNATIVE_SCREEN.as_bytes())?;
        self.output.flush()?;
        if let Some(original) = &self.original {
            tcsetattr(&self.input, OptionalActions::Now, original).map_err(io::Error::from)
        } else {
            Ok(())
        }
    }
}
