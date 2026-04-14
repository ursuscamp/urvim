use super::*;

#[allow(dead_code)]
impl<I: Read + AsFd, O: Write + AsFd> Terminal<I, O> {
    pub fn read_event(&mut self) -> io::Result<Event> {
        loop {
            if self.paste_active {
                return self.read_paste_event();
            }

            if self.buffer.filled_len() > 0 {
                let data = self.buffer.get_range(0, self.buffer.len());
                if data.starts_with(b"\x1b[200~") {
                    let remaining = self.buffer.get_range(6, self.buffer.len()).to_vec();
                    self.buffer.clear();
                    self.buffer.extend(&remaining);
                    self.paste_active = true;
                    return self.read_paste_event();
                }

                let event = escape::parse_event_with_buffer(&mut self.buffer);
                return Ok(event);
            }

            if let Some((rows, cols)) = get_terminal_size()
                && (rows != self.last_rows || cols != self.last_cols)
            {
                self.last_rows = rows;
                self.last_cols = cols;
                return Ok(Event::Resize(rows, cols));
            }

            if self.is_tty {
                let input_fd = self.input.as_fd();
                let poll_fd = PollFd::new(&input_fd, PollFlags::IN);
                let mut fds = [poll_fd];
                let poll_result = poll(&mut fds, POLL_TIMEOUT_MS);

                match poll_result {
                    Ok(n) if n > 0 && fds[0].revents().contains(PollFlags::IN) => {
                        let mut buf = [0u8; 64];
                        match self.input.read(&mut buf) {
                            Ok(0) => return Ok(KeyCode::Null.event()),
                            Ok(bytes_read) => {
                                self.buffer.clear();
                                for &b in &buf[..bytes_read] {
                                    self.buffer.push(b);
                                }
                                return self.process_buffer_for_event();
                            }
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(_) => return Ok(Event::Tick),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
            } else {
                let mut buf = [0u8; 64];
                match self.input.read(&mut buf) {
                    Ok(0) => return Ok(KeyCode::Null.event()),
                    Ok(bytes_read) => {
                        self.buffer.clear();
                        for &b in &buf[..bytes_read] {
                            self.buffer.push(b);
                        }
                        return self.process_buffer_for_event();
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
    }

    fn process_buffer_for_event(&mut self) -> io::Result<Event> {
        let data = self.buffer.get_range(0, self.buffer.len());
        if data.starts_with(b"\x1b[200~") {
            let remaining = self.buffer.get_range(6, self.buffer.len()).to_vec();
            self.buffer.clear();
            self.buffer.extend(&remaining);
            self.paste_active = true;
            return self.read_paste_event();
        }

        let event = escape::parse_event_with_buffer(&mut self.buffer);
        Ok(event)
    }

    fn read_paste_event(&mut self) -> io::Result<Event> {
        let paste_end_marker = b"\x1b[201~";
        let mut temp_buf = [0u8; 256];

        loop {
            if self.buffer.len() > MAX_PASTE_SIZE {
                self.buffer.clear();
                self.paste_active = false;
                return self.read_event();
            }

            if self.buffer.len() >= 6 {
                let data = self.buffer.get_range(0, self.buffer.len());
                if let Some(pos) = data.windows(6).position(|w| w == paste_end_marker) {
                    let content_bytes = self.buffer.get_range(0, pos);
                    let paste_content = String::from_utf8_lossy(content_bytes).into_owned();
                    let remaining_start = pos + 6;
                    let remaining = self
                        .buffer
                        .get_range(remaining_start, self.buffer.len())
                        .to_vec();
                    self.buffer.clear();
                    self.buffer.extend(&remaining);
                    self.paste_active = false;

                    return Ok(Event::Paste(paste_content));
                }
            }

            match self.input.read(&mut temp_buf) {
                Ok(0) => {
                    self.buffer.clear();
                    self.paste_active = false;
                    return self.read_event();
                }
                Ok(n) => {
                    for &b in &temp_buf[..n] {
                        self.buffer.push(b);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.buffer.clear();
                    self.paste_active = false;
                    return self.read_event();
                }
            }
        }
    }
}
