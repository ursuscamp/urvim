use super::{InlayHintState, Layout, LayoutNode, PaneId, PaneNode, SplitNode};
use crate::session::{SessionFile, SessionNode, SessionPane, SessionSplit, SessionSplitSize};

impl Layout {
    /// Converts a live layout into serializable session state.
    pub fn to_session(&self) -> SessionFile {
        let root = Self::node_to_session(self.root.as_ref().expect("layout root should exist"))
            .expect("session should contain a buffer pane");
        SessionFile {
            version: crate::session::session_version(),
            cwd: std::env::current_dir()
                .map(|cwd| cwd.display().to_string())
                .unwrap_or_default(),
            label: crate::session::current_session_label().unwrap_or_else(|| "cwd".to_string()),
            focused_pane: self.focused_pane.0,
            root,
        }
    }

    /// Restores a live layout from serializable session state.
    pub fn from_session(session: SessionFile) -> Self {
        let root = Self::node_from_session(session.root);
        let focused_pane = PaneId(session.focused_pane);
        let mut layout = Self {
            root: Some(root),
            focused_pane,
            last_editor_pane: focused_pane,
            next_pane_id: 0,
            status_bar: crate::status_bar::StatusBar::new(),
            origin: Default::default(),
            size: Default::default(),
            dialogs: super::Dialogs::default(),
            jobs: std::sync::Arc::new(crate::background::JobManager::new()),
            inlay_hints: InlayHintState::Idle,
            autocomplete: super::AutocompleteState::default(),
            plugin_windows: super::PluginWindowManager::new(),
            plugin_pane_inherited_keymap: crate::editor::InheritedKeymap::new(
                crate::editor::NormalMode::keymap(),
            ),
            plugin_pane_key_sequence: super::PluginPaneKeySequence::None,
            modal_inherited_keymap: crate::editor::InheritedKeymap::new(
                crate::editor::NormalMode::keymap(),
            ),
            modal_key_sequence: super::ModalKeySequence::None,
            insert_session: None,
        };
        layout.next_pane_id = layout.max_pane_id().map(|id| id.0 + 1).unwrap_or(0);
        if layout.root.is_some() {
            layout.focus_pane(focused_pane);
        }
        layout.emit_initial_lifecycle_events();
        layout
    }

    fn node_to_session(node: &LayoutNode) -> Option<SessionNode> {
        match node {
            LayoutNode::Pane(pane) => pane.editor_window_group().map(|window_group| {
                SessionNode::Pane(SessionPane {
                    pane_id: pane.id.0,
                    window_group: window_group.to_session(),
                })
            }),
            LayoutNode::Split(split) => {
                let first = Self::node_to_session(&split.first);
                let second = Self::node_to_session(&split.second);
                match (first, second) {
                    (Some(first), Some(second)) => Some(SessionNode::Split(SessionSplit {
                        axis: split.axis,
                        split_size: SessionSplitSize {
                            first_weight: split.split_size.first_weight(),
                            second_weight: split.split_size.second_weight(),
                        },
                        last_focused_pane: split.last_focused_pane.0,
                        first: Box::new(first),
                        second: Box::new(second),
                    })),
                    (Some(first), None) | (None, Some(first)) => Some(first),
                    (None, None) => None,
                }
            }
        }
    }

    fn node_from_session(node: SessionNode) -> LayoutNode {
        match node {
            SessionNode::Pane(pane) => LayoutNode::Pane(PaneNode::new_editor(
                PaneId(pane.pane_id),
                crate::window_group::WindowGroup::from_session(pane.window_group),
            )),
            SessionNode::Split(split) => LayoutNode::Split(SplitNode::new(
                split.axis,
                Self::node_from_session(*split.first),
                Self::node_from_session(*split.second),
                PaneId(split.last_focused_pane),
            )),
        }
    }

    fn max_pane_id(&self) -> Option<PaneId> {
        fn visit(node: &LayoutNode, max_id: &mut Option<PaneId>) {
            match node {
                LayoutNode::Pane(pane) => {
                    if max_id.map(|id| pane.id.0 > id.0).unwrap_or(true) {
                        *max_id = Some(pane.id);
                    }
                }
                LayoutNode::Split(split) => {
                    visit(&split.first, max_id);
                    visit(&split.second, max_id);
                }
            }
        }

        let mut max_id = None;
        if let Some(root) = self.root.as_ref() {
            visit(root, &mut max_id);
        }
        max_id
    }
}
