use super::WindowGroup;
use crate::session::SessionWindowGroup;
use std::path::Path;

impl WindowGroup {
    /// Converts a live window group into serializable session state.
    pub fn to_session(&self) -> SessionWindowGroup {
        SessionWindowGroup {
            active_tab: self.active_tab_index(),
            tabs: self.tabs.iter().map(|window| window.to_session()).collect(),
        }
    }

    /// Restores a live window group from serialized session state.
    pub fn from_session(session: SessionWindowGroup) -> Self {
        let mut tabs = Vec::new();

        for tab in session.tabs {
            let path = Path::new(&tab.path);
            if !path.exists() {
                tracing::warn!(path = %path.display(), "skipping missing session buffer");
                continue;
            }

            let Ok(buffer_id) = crate::globals::with_buffer_pool(|pool| pool.open_buffer(path))
            else {
                continue;
            };

            tabs.push(crate::window::Window::from_session(tab, buffer_id));
        }

        let mut group = Self::new(tabs);
        if !group.tabs.is_empty() {
            group.active_tab = session.active_tab.min(group.tabs.len().saturating_sub(1));
        }
        group
    }
}
