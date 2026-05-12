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
    /// Syntax cache refresh for a specific buffer.
    SyntaxRefresh(BufferId),
    /// Indent scope cache refresh for a specific buffer.
    IndentScopeRefresh(BufferId),
    /// File picker search.
    FilePickerSearch,
    /// Live grep picker search.
    GrepPickerSearch,
    /// Document symbol picker search.
    DocSymbolsPickerSearch,
    /// Workspace symbol picker search.
    WorkspaceSymbolsPickerSearch,
    /// Picker preview syntax refresh.
    PickerPreviewSyntax,
    /// LSP rename.
    LspRename(BufferId),
    /// Test-only gate job.
    #[cfg(test)]
    TestGate,
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SyntaxRefresh(buffer_id) => {
                write!(f, "syntax-refresh:{}", buffer_id.get())
            }
            Self::IndentScopeRefresh(buffer_id) => {
                write!(f, "indent-scope-refresh:{}", buffer_id.get())
            }
            Self::FilePickerSearch => f.write_str("file-picker-search"),
            Self::GrepPickerSearch => f.write_str("grep-picker-search"),
            Self::DocSymbolsPickerSearch => f.write_str("doc-symbols-picker-search"),
            Self::WorkspaceSymbolsPickerSearch => f.write_str("workspace-symbols-picker-search"),
            Self::PickerPreviewSyntax => f.write_str("picker-preview-syntax"),
            Self::LspRename(buffer_id) => write!(f, "lsp-rename:{}", buffer_id.get()),
            #[cfg(test)]
            Self::TestGate => f.write_str("gate"),
        }
    }
}
