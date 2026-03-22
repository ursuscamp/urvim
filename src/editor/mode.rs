use super::HandleKeyResult;
use crate::terminal::{CursorStyle, Key};

pub trait Mode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult;
    fn cursor_style(&self) -> CursorStyle;
    fn is_waiting(&self) -> bool;
    fn clear_buffer(&mut self);
}
