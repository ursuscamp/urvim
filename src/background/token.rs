use crate::buffer::BufferId;
use std::fmt;

/// A generation token used to reject stale job results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobToken {
    generation: u64,
}

impl JobToken {
    /// Creates a new generation token.
    pub fn new(generation: u64) -> Self {
        Self { generation }
    }

    /// Returns the numeric generation value.
    pub fn generation(self) -> u64 {
        self.generation
    }
}

/// Identifies the kind of work a job performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum JobKind {
    /// Buffer cache refresh for a specific buffer.
    BufferCacheRefresh(BufferId),
    /// File picker search.
    FilePickerSearch,
    /// Live grep picker search.
    GrepPickerSearch,
    /// Picker preview syntax refresh.
    PickerPreviewSyntax,
    /// Test-only gate job.
    #[cfg(test)]
    TestGate,
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferCacheRefresh(buffer_id) => {
                write!(f, "buffer-cache:{}", buffer_id.get())
            }
            Self::FilePickerSearch => f.write_str("file-picker-search"),
            Self::GrepPickerSearch => f.write_str("grep-picker-search"),
            Self::PickerPreviewSyntax => f.write_str("picker-preview-syntax"),
            #[cfg(test)]
            Self::TestGate => f.write_str("gate"),
        }
    }
}
