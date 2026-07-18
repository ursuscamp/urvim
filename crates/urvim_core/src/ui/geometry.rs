//! Shared terminal UI geometry types.

/// Zero-based terminal cell position.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    /// Row from the top edge.
    pub row: u16,
    /// Column from the left edge.
    pub col: u16,
}

impl Position {
    /// Creates a terminal position.
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

/// Terminal rectangle dimensions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Size {
    /// Height in rows.
    pub rows: u16,
    /// Width in columns.
    pub cols: u16,
}

impl Size {
    /// Creates terminal dimensions.
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}
