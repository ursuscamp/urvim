use super::{Position, Size};

impl Position {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

impl Size {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}
