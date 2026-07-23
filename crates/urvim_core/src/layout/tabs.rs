//! Tab inspection and mutation helpers.

use super::{Layout, Pane, PaneId};
use crate::buffer::BufferId;
use crate::editor_tab::TabId;

/// Snapshot of a tab and its containing editor pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabSnapshot {
    /// Stable runtime tab identifier.
    pub id: TabId,
    /// Pane containing the tab.
    pub pane_id: PaneId,
    /// Buffer shown by the tab.
    pub buffer_id: BufferId,
    /// Zero-based position in the pane's tab list.
    pub index: usize,
    /// Whether the tab is active in its pane.
    pub active: bool,
}

impl Layout {
    /// Returns tab metadata in layout and tab order, optionally for one pane.
    pub fn tab_snapshots(&self, pane_id: Option<PaneId>) -> Result<Vec<TabSnapshot>, String> {
        if let Some(pane_id) = pane_id {
            match self.pane_kind(pane_id) {
                Some(super::PaneKind::Editor) => {}
                Some(super::PaneKind::Plugin) => {
                    return Err(format!("pane_id {} is not an editor pane", pane_id.0));
                }
                None => return Err(format!("unknown pane_id {}", pane_id.0)),
            }
        }

        let Some(root) = self.root.as_ref() else {
            return Ok(Vec::new());
        };
        let pane_ids = pane_id.map_or_else(|| self.editor_pane_ids(), |id| vec![id]);
        let mut snapshots = Vec::new();
        for pane_id in pane_ids {
            let editor_pane = Self::find_pane(root, pane_id)
                .and_then(Pane::editor_pane)
                .expect("validated editor pane should exist");
            let active = editor_pane.active_tab_id();
            snapshots.extend(
                editor_pane
                    .tab_ids_and_buffer_ids()
                    .into_iter()
                    .enumerate()
                    .map(|(index, (id, buffer_id))| TabSnapshot {
                        id,
                        pane_id,
                        buffer_id,
                        index,
                        active: active == Some(id),
                    }),
            );
        }
        Ok(snapshots)
    }

    /// Returns the last-focused editor pane identifier.
    pub fn last_editor_pane_id(&self) -> PaneId {
        self.last_editor_pane
    }

    /// Returns the active tab identifier for an editor pane.
    pub fn active_tab_id_for_pane(&self, pane_id: PaneId) -> Result<Option<TabId>, String> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| format!("unknown pane_id {}", pane_id.0))?;
        let pane = Self::find_pane(root, pane_id)
            .ok_or_else(|| format!("unknown pane_id {}", pane_id.0))?;
        let editor_pane = pane
            .editor_pane()
            .ok_or_else(|| format!("pane_id {} is not an editor pane", pane_id.0))?;
        Ok(editor_pane.active_tab_id())
    }

    /// Returns the pane and buffer associated with a tab.
    pub fn tab_location(&self, tab_id: TabId) -> Option<(PaneId, BufferId)> {
        self.tab_snapshots(None)
            .ok()?
            .into_iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| (tab.pane_id, tab.buffer_id))
    }

    /// Focuses and activates a tab.
    pub fn activate_tab(&mut self, tab_id: TabId) -> Result<(), String> {
        self.with_event_transition(|layout| {
            let (pane_id, _) = layout
                .tab_location(tab_id)
                .ok_or_else(|| unknown_tab_error(tab_id))?;
            let root = layout.root.as_mut().expect("known tab requires a root");
            let editor_pane = Self::find_pane_mut(root, pane_id)
                .and_then(Pane::editor_pane_mut)
                .expect("known tab should belong to an editor pane");
            editor_pane.activate_tab(tab_id);
            layout.focus_pane(pane_id);
            Ok(())
        })
    }

    /// Closes a tab and prunes its pane if it becomes empty.
    pub fn close_tab(&mut self, tab_id: TabId) -> Result<(), String> {
        self.with_event_transition(|layout| {
            let (pane_id, _) = layout
                .tab_location(tab_id)
                .ok_or_else(|| unknown_tab_error(tab_id))?;
            let root = layout.root.as_mut().expect("known tab requires a root");
            let editor_pane = Self::find_pane_mut(root, pane_id)
                .and_then(Pane::editor_pane_mut)
                .expect("known tab should belong to an editor pane");
            editor_pane.remove_tab(tab_id);
            layout.ensure_editor_pane_has_tab();
            layout.prune_empty_panes();
            Ok(())
        })
    }

    /// Moves a tab to another editor pane, preserving its runtime identity and view state.
    pub fn move_tab(&mut self, tab_id: TabId, target_pane_id: PaneId) -> Result<(), String> {
        self.with_event_transition(|layout| {
            let (source_pane_id, _) = layout
                .tab_location(tab_id)
                .ok_or_else(|| unknown_tab_error(tab_id))?;
            match layout.pane_kind(target_pane_id) {
                Some(super::PaneKind::Editor) => {}
                Some(super::PaneKind::Plugin) => {
                    return Err(format!(
                        "pane_id {} is not an editor pane",
                        target_pane_id.0
                    ));
                }
                None => return Err(format!("unknown pane_id {}", target_pane_id.0)),
            }

            if source_pane_id == target_pane_id {
                let root = layout.root.as_mut().expect("known tab requires a root");
                Self::find_pane_mut(root, source_pane_id)
                    .and_then(Pane::editor_pane_mut)
                    .expect("known tab should belong to an editor pane")
                    .activate_tab(tab_id);
                layout.focus_pane(target_pane_id);
                return Ok(());
            }

            let tab = {
                let root = layout.root.as_mut().expect("known tab requires a root");
                Self::find_pane_mut(root, source_pane_id)
                    .and_then(Pane::editor_pane_mut)
                    .expect("known tab should belong to an editor pane")
                    .remove_tab(tab_id)
                    .expect("known tab should be removable")
            };
            {
                let root = layout.root.as_mut().expect("target pane requires a root");
                Self::find_pane_mut(root, target_pane_id)
                    .and_then(Pane::editor_pane_mut)
                    .expect("validated target should be an editor pane")
                    .insert_tab(tab);
            }
            layout.prune_empty_panes();
            layout.focus_pane(target_pane_id);
            Ok(())
        })
    }
}

fn unknown_tab_error(tab_id: TabId) -> String {
    format!("unknown tab_id {}", tab_id.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::editor_pane::EditorPane;
    use crate::ui::{Command, Intent};

    #[test]
    fn activates_and_closes_tabs_by_stable_id() {
        let _lock = crate::globals::buffer_pool_test_lock();
        let mut layout = Layout::new(EditorPane::from_buffers(vec![
            Buffer::from_str("first"),
            Buffer::from_str("second"),
        ]));
        let tabs = layout.tab_snapshots(None).unwrap();

        layout.activate_tab(tabs[1].id).unwrap();
        assert_eq!(
            layout.active_tab_id_for_pane(tabs[1].pane_id).unwrap(),
            Some(tabs[1].id)
        );

        layout.close_tab(tabs[0].id).unwrap();
        let remaining = layout.tab_snapshots(None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, tabs[1].id);
    }

    #[test]
    fn moves_tab_between_editor_panes_without_changing_identity() {
        let _lock = crate::globals::buffer_pool_test_lock();
        let mut layout = Layout::new(EditorPane::from_buffers(vec![
            Buffer::from_str("first"),
            Buffer::from_str("second"),
        ]));
        let moved = layout.tab_snapshots(None).unwrap()[1];
        assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));
        let target = layout.last_editor_pane_id();

        layout.move_tab(moved.id, target).unwrap();

        let moved_after = layout
            .tab_snapshots(None)
            .unwrap()
            .into_iter()
            .find(|tab| tab.id == moved.id)
            .unwrap();
        assert_eq!(moved_after.pane_id, target);
        assert_eq!(moved_after.buffer_id, moved.buffer_id);
        assert!(moved_after.active);
    }

    #[test]
    fn rejects_plugin_panes_as_move_targets_before_removing_tab() {
        let _lock = crate::globals::buffer_pool_test_lock();
        let mut layout = Layout::new(EditorPane::from_buffers(vec![Buffer::from_str("first")]));
        let tab = layout.tab_snapshots(None).unwrap()[0];
        let plugin_pane = layout
            .create_plugin_pane(
                "demo".to_string(),
                None,
                super::super::SplitAxis::Vertical,
                super::super::SplitSize::even(),
                crate::ui::plugin_pane::PluginPaneOptions::default(),
            )
            .unwrap();

        let error = layout.move_tab(tab.id, plugin_pane).unwrap_err();

        assert_eq!(
            error,
            format!("pane_id {} is not an editor pane", plugin_pane.0)
        );
        assert!(layout.tab_location(tab.id).is_some());
    }
}
