use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct TestBackend {
    input: Arc<Mutex<VecDeque<u8>>>,
    pub output: Arc<Mutex<Vec<u8>>>,
}

impl TestBackend {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            input: Arc::new(Mutex::new(VecDeque::from(data))),
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_output(&self) -> Vec<u8> {
        self.output.lock().unwrap().clone()
    }
}

impl Read for TestBackend {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut input = self.input.lock().unwrap();
        if input.is_empty() {
            return Ok(0);
        }
        let mut i = 0;
        while i < buf.len() {
            match input.pop_front() {
                Some(b) => {
                    buf[i] = b;
                    i += 1;
                }
                None => break,
            }
        }
        Ok(i)
    }
}

impl Write for TestBackend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut output = self.output.lock().unwrap();
        output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for TestBackend {
    fn as_fd(&self) -> rustix::fd::BorrowedFd<'_> {
        panic!("TestBackend does not have a valid file descriptor")
    }
}
