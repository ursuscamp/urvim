use crate::layout::SplitAxis;
use serde::{Deserialize, Serialize};

/// Serialized session file root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFile {
    pub version: u32,
    pub cwd: String,
    pub label: String,
    pub focused_pane: usize,
    pub root: SessionNode,
}

/// Serialized recursive layout node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SessionNode {
    Pane(SessionPane),
    Split(SessionSplit),
}

/// Serialized pane leaf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPane {
    pub pane_id: usize,
    pub editor_pane: SessionEditorPane,
}

/// Serialized split node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSplit {
    pub axis: SplitAxis,
    pub split_size: SessionSplitSize,
    pub last_focused_pane: usize,
    pub first: Box<SessionNode>,
    pub second: Box<SessionNode>,
}

/// Serialized split ratio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSplitSize {
    pub first_weight: u16,
    pub second_weight: u16,
}

/// Serialized editor-pane state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionEditorPane {
    pub active_tab: usize,
    pub tabs: Vec<SessionEditorTab>,
}

/// Serialized per-tab state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionEditorTab {
    pub path: String,
    pub cursor: SessionCursor,
    pub scroll_offset: SessionPosition,
    pub wrapped_row_offset: u16,
    pub wrap_enabled: bool,
}

/// Serialized cursor position.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionCursor {
    pub row: usize,
    pub col: usize,
}

/// Serialized scroll position.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPosition {
    pub row: u16,
    pub col: u16,
}
