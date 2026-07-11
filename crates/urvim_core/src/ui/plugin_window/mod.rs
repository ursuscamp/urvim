//! Retained floating windows owned by plugins.

use crate::editor::{InheritedKeymap, NormalMode, TrieKeymap};
use crate::screen::Screen;
use crate::ui::floating_window::{
    FloatingAnchor, FloatingMargins, FloatingPlacement, FloatingWindowFrame,
    FloatingWindowFrameLabel,
};
use crate::ui::{
    FocusPolicy, Intent, KeymapInheritance, UiContext, UiEvent, UiEventResult, UiRect,
};
use crate::widget::Widget;
use crate::{editor, globals};
use std::collections::BTreeMap;
use urvim_terminal::Key;
use urvim_theme::Tag;

/// Stable identifier for a plugin-owned floating window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginWindowId(pub usize);

/// Configuration for a plugin floating window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWindowOptions {
    /// Placement mode.
    pub placement: FloatingPlacement,
    /// Content height in terminal rows.
    pub rows: u16,
    /// Content width in terminal columns.
    pub cols: u16,
    /// Optional border title.
    pub title: Option<String>,
    /// Theme tag for the window body.
    pub body_style: Tag,
    /// Theme tag for the unfocused window border.
    pub border_style: Tag,
    /// Theme tag for the focused window border.
    pub focused_border_style: Tag,
}

impl Default for PluginWindowOptions {
    fn default() -> Self {
        Self {
            placement: FloatingPlacement::Anchored {
                anchor: FloatingAnchor::Center,
                margins: FloatingMargins::default(),
            },
            rows: 8,
            cols: 40,
            title: None,
            body_style: Tag::parse("ui.window").expect("built-in window tag should parse"),
            border_style: Tag::parse("ui.window.lines.border")
                .expect("built-in border tag should parse"),
            focused_border_style: Tag::parse("ui.window.lines.resize")
                .expect("built-in focused border tag should parse"),
        }
    }
}

/// A styled text segment in a plugin window line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWindowSegment {
    /// Segment text without a newline.
    pub text: String,
    /// Optional theme tag overlaid on the window body style.
    pub style: Option<Tag>,
}

/// Retained content for a plugin floating window.
pub type PluginWindowContent = Vec<Vec<PluginWindowSegment>>;

#[derive(Debug, Clone)]
struct KeyBinding {
    rhs: String,
    intent: Intent,
}

/// A plugin-owned floating window and its retained UI state.
#[derive(Debug)]
pub struct PluginWindow {
    owner: String,
    options: PluginWindowOptions,
    content: PluginWindowContent,
    visible: bool,
    keymaps: TrieKeymap<KeyBinding>,
    pending_keys: Vec<String>,
}

impl PluginWindow {
    /// Creates a retained plugin-owned window.
    pub fn new(owner: String, options: PluginWindowOptions) -> Self {
        Self {
            owner,
            options,
            content: Vec::new(),
            visible: true,
            keymaps: TrieKeymap::<KeyBinding>::new(),
            pending_keys: Vec::new(),
        }
    }

    /// Returns the plugin that owns this window.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Returns the current window configuration.
    pub fn options(&self) -> &PluginWindowOptions {
        &self.options
    }

    /// Returns the current retained content.
    pub fn content(&self) -> &PluginWindowContent {
        &self.content
    }

    /// Returns whether this window is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Updates the window's presentation options.
    pub fn set_options(&mut self, options: PluginWindowOptions) {
        self.options = options;
    }

    /// Replaces the retained window content.
    pub fn set_content(&mut self, content: PluginWindowContent) {
        self.content = content;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Installs a local keymap binding.
    pub fn set_keymap(&mut self, keys: Vec<String>, rhs: String, intent: Intent) {
        self.keymaps
            .insert_sequence(keys, KeyBinding { rhs, intent });
        self.pending_keys.clear();
    }

    /// Removes a local keymap binding.
    pub fn delete_keymap(&mut self, keys: &[String]) {
        self.keymaps.remove_sequence(keys);
        self.pending_keys.clear();
    }

    /// Returns all local keymap bindings.
    pub fn keymaps(&self) -> Vec<(Vec<String>, String)> {
        self.keymaps
            .bindings()
            .into_iter()
            .map(|(keys, binding)| (keys, binding.rhs.clone()))
            .collect()
    }

    /// Routes a key through the local keymap.
    pub fn handle_key(&mut self, key: &Key) -> editor::HandleKeyResult {
        self.pending_keys.push(key.canonical_string());
        if let Some(binding) = self.keymaps.get(&self.pending_keys) {
            let intent = binding.intent.clone();
            self.pending_keys.clear();
            return editor::HandleKeyResult::Complete(intent);
        }

        if self.keymaps.is_prefix(&self.pending_keys) {
            return editor::HandleKeyResult::WaitForMore;
        }

        self.pending_keys.clear();
        editor::HandleKeyResult::InvalidSequence
    }

    /// Clears any partially entered local key sequence.
    pub fn clear_pending_keys(&mut self) {
        self.pending_keys.clear();
    }

    fn has_explicit_escape_binding(&self) -> bool {
        let escape = vec!["<Esc>".to_string()];
        self.keymaps.is_prefix(&escape)
    }

    /// Renders retained content across a rectangle using the configured body style.
    pub fn render_content_in_rect(&self, screen: &mut Screen, rect: UiRect) {
        if rect.size.rows == 0 || rect.size.cols == 0 {
            return;
        }
        let body_style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default(self.options.body_style.as_str()))
                .unwrap_or_default()
        });
        screen.fill_region(
            rect.origin.row,
            rect.origin.col,
            rect.size.rows,
            rect.size.cols,
            body_style,
        );

        for (row_offset, line) in self
            .content
            .iter()
            .take(rect.size.rows as usize)
            .enumerate()
        {
            let row = rect.origin.row + row_offset as u16;
            let mut col = rect.origin.col;
            let right_col = rect.origin.col + rect.size.cols;
            for segment in line {
                if col >= right_col {
                    break;
                }
                let style = segment
                    .style
                    .as_ref()
                    .map(|tag| {
                        globals::with_active_theme(|theme| {
                            theme
                                .map(|theme| theme.resolve_name_with_default(tag.as_str()))
                                .unwrap_or_default()
                        })
                    })
                    .unwrap_or_default();
                let style = body_style.overlay(style);
                let remaining = usize::from(right_col - col);
                let clipped = crate::ui::text_width::clip_text(
                    segment.text.as_str(),
                    remaining,
                    crate::ui::text_width::ClipSide::Start,
                );
                screen.write_string(row, col, style, clipped.text.as_str());
                col = col.saturating_add(clipped.width as u16);
            }
        }
    }

    fn render_in_rect(&mut self, screen: &mut Screen, rect: UiRect, focused: bool) {
        let Some(frame) = FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            self.options.rows,
            self.options.cols,
            self.options.placement,
        ) else {
            return;
        };
        self.render_frame(screen, frame, focused);
    }

    fn render_frame(&mut self, screen: &mut Screen, frame: FloatingWindowFrame, focused: bool) {
        let (body_style, border_style) = globals::with_active_theme(|theme| {
            let body = theme
                .map(|theme| theme.resolve_name_with_default(self.options.body_style.as_str()))
                .unwrap_or_default();
            let border = theme
                .map(|theme| {
                    if focused {
                        theme.resolve_name_with_default(self.options.focused_border_style.as_str())
                    } else {
                        theme.resolve_name_with_default(self.options.border_style.as_str())
                    }
                })
                .unwrap_or_default();
            (body, border)
        });
        frame.render_bordered_with_label(
            screen,
            border_style,
            body_style,
            self.options
                .title
                .as_deref()
                .map(FloatingWindowFrameLabel::top_center),
        );

        self.render_content_in_rect(
            screen,
            UiRect::new(frame.content_origin, frame.content_size),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginWindowKeySequence {
    None,
    Local,
    Inherited,
}

/// Retained registry of plugin floating windows.
#[derive(Debug)]
pub struct PluginWindowManager {
    windows: BTreeMap<PluginWindowId, PluginWindow>,
    next_id: usize,
    focused: Option<PluginWindowId>,
    inherited_keymap: InheritedKeymap,
    key_sequence: PluginWindowKeySequence,
}

impl Default for PluginWindowManager {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            next_id: 0,
            focused: None,
            inherited_keymap: InheritedKeymap::new(NormalMode::keymap()),
            key_sequence: PluginWindowKeySequence::None,
        }
    }
}

impl PluginWindowManager {
    /// Creates an empty plugin-window registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all currently registered window IDs.
    pub fn ids(&self) -> impl Iterator<Item = PluginWindowId> + '_ {
        self.windows.keys().copied()
    }

    /// Returns the IDs of all visible plugin windows in stable creation order.
    pub fn visible_ids(&self) -> impl Iterator<Item = PluginWindowId> + '_ {
        self.windows
            .iter()
            .filter_map(|(id, window)| window.is_visible().then_some(*id))
    }

    /// Returns the currently focused plugin window, if any.
    pub fn focused(&self) -> Option<PluginWindowId> {
        self.focused
    }

    /// Creates a visible, unfocused plugin window.
    pub fn create(&mut self, owner: String, options: PluginWindowOptions) -> PluginWindowId {
        let id = PluginWindowId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.windows.insert(id, PluginWindow::new(owner, options));
        id
    }

    /// Returns a window owned by `owner` or an ownership error.
    pub fn owned_window(&self, owner: &str, id: PluginWindowId) -> Result<&PluginWindow, String> {
        let window = self
            .windows
            .get(&id)
            .ok_or_else(|| format!("unknown plugin window_id {}", id.0))?;
        if window.owner() != owner {
            return Err(format!("plugin {owner:?} does not own window_id {}", id.0));
        }
        Ok(window)
    }

    /// Returns a mutable window owned by `owner` or an ownership error.
    pub fn owned_window_mut(
        &mut self,
        owner: &str,
        id: PluginWindowId,
    ) -> Result<&mut PluginWindow, String> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or_else(|| format!("unknown plugin window_id {}", id.0))?;
        if window.owner() != owner {
            return Err(format!("plugin {owner:?} does not own window_id {}", id.0));
        }
        Ok(window)
    }

    /// Updates a window's configuration.
    pub fn configure(
        &mut self,
        owner: &str,
        id: PluginWindowId,
        options: PluginWindowOptions,
    ) -> Result<(), String> {
        self.owned_window_mut(owner, id)?.set_options(options);
        Ok(())
    }

    /// Replaces a window's retained content.
    pub fn set_content(
        &mut self,
        owner: &str,
        id: PluginWindowId,
        content: PluginWindowContent,
    ) -> Result<(), String> {
        self.owned_window_mut(owner, id)?.set_content(content);
        Ok(())
    }

    /// Shows a window.
    pub fn show(&mut self, owner: &str, id: PluginWindowId) -> Result<(), String> {
        self.owned_window_mut(owner, id)?.set_visible(true);
        Ok(())
    }

    /// Hides a window and clears focus if necessary.
    pub fn hide(&mut self, owner: &str, id: PluginWindowId) -> Result<(), String> {
        self.owned_window_mut(owner, id)?.set_visible(false);
        if self.focused == Some(id) {
            self.blur_focused();
        }
        Ok(())
    }

    /// Focuses a visible window.
    pub fn focus(&mut self, owner: &str, id: PluginWindowId) -> Result<(), String> {
        let window = self.owned_window(owner, id)?;
        if !window.is_visible() {
            return Err(format!("plugin window_id {} is hidden", id.0));
        }
        self.focus_id(id);
        Ok(())
    }

    /// Focuses a visible plugin window by ID for layout-level focus traversal.
    pub fn focus_id(&mut self, id: PluginWindowId) -> bool {
        let Some(window) = self.windows.get(&id) else {
            return false;
        };
        if !window.is_visible() {
            return false;
        }
        self.focused = Some(id);
        self.clear_pending_keys();
        true
    }

    /// Clears plugin-window focus without requiring an owning plugin.
    pub fn blur_focused(&mut self) {
        self.focused = None;
        self.clear_pending_keys();
    }

    /// Clears plugin-window focus.
    pub fn blur(&mut self, owner: &str, id: PluginWindowId) -> Result<(), String> {
        self.owned_window(owner, id)?;
        if self.focused == Some(id) {
            self.blur_focused();
        }
        Ok(())
    }

    /// Closes a window and clears focus if necessary.
    pub fn close(&mut self, owner: &str, id: PluginWindowId) -> Result<(), String> {
        self.owned_window(owner, id)?;
        self.windows.remove(&id);
        if self.focused == Some(id) {
            self.blur_focused();
        }
        Ok(())
    }

    /// Closes all windows owned by a plugin.
    pub fn close_owned(&mut self, owner: &str) {
        let ids: Vec<_> = self
            .windows
            .iter()
            .filter_map(|(id, window)| (window.owner() == owner).then_some(*id))
            .collect();
        for id in ids {
            self.windows.remove(&id);
            if self.focused == Some(id) {
                self.blur_focused();
            }
        }
    }

    /// Installs a command binding for a window.
    pub fn set_keymap(
        &mut self,
        owner: &str,
        id: PluginWindowId,
        keys: Vec<String>,
        rhs: String,
        intent: Intent,
    ) -> Result<(), String> {
        self.owned_window_mut(owner, id)?
            .set_keymap(keys, rhs, intent);
        if self.focused == Some(id) {
            self.clear_pending_keys();
        }
        Ok(())
    }

    /// Removes a command binding from a window.
    pub fn delete_keymap(
        &mut self,
        owner: &str,
        id: PluginWindowId,
        keys: &[String],
    ) -> Result<(), String> {
        self.owned_window_mut(owner, id)?.delete_keymap(keys);
        if self.focused == Some(id) {
            self.clear_pending_keys();
        }
        Ok(())
    }

    /// Returns a plugin window's configured keymaps.
    pub fn keymaps(
        &self,
        owner: &str,
        id: PluginWindowId,
    ) -> Result<Vec<(Vec<String>, String)>, String> {
        let window = self.owned_window(owner, id)?;
        Ok(window.keymaps())
    }

    /// Routes an event to the focused plugin window.
    pub fn route_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(id) = self.focused else {
            return UiEventResult::NotHandled;
        };

        if !self.windows.contains_key(&id) {
            self.focused = None;
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => {
                // An inherited prefix owns the rest of its sequence; otherwise
                // window-local mappings always receive the key first.
                if self.key_sequence == PluginWindowKeySequence::Inherited {
                    return self.route_inherited_key(key);
                }

                let Some(window) = self.windows.get_mut(&id) else {
                    self.focused = None;
                    return UiEventResult::NotHandled;
                };
                if key.canonical_string() == "<Esc>" && !window.has_explicit_escape_binding() {
                    window.pending_keys.clear();
                    self.blur_focused();
                    return UiEventResult::Handled(Vec::new());
                }

                match window.handle_key(key) {
                    editor::HandleKeyResult::Complete(intent) => {
                        self.key_sequence = PluginWindowKeySequence::None;
                        UiEventResult::Handled(vec![intent])
                    }
                    editor::HandleKeyResult::WaitForMore => {
                        self.key_sequence = PluginWindowKeySequence::Local;
                        UiEventResult::Handled(Vec::new())
                    }
                    editor::HandleKeyResult::InvalidSequence
                        if self.key_sequence == PluginWindowKeySequence::Local =>
                    {
                        self.key_sequence = PluginWindowKeySequence::None;
                        UiEventResult::Handled(Vec::new())
                    }
                    editor::HandleKeyResult::InvalidSequence => self.route_inherited_key(key),
                }
            }
            UiEvent::Paste(_) | UiEvent::Resize(_, _) => {
                self.clear_pending_keys();
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    fn clear_pending_keys(&mut self) {
        self.inherited_keymap.clear_pending();
        self.key_sequence = PluginWindowKeySequence::None;
        if let Some(window) = self.focused.and_then(|id| self.windows.get_mut(&id)) {
            window.clear_pending_keys();
        }
    }

    fn route_inherited_key(&mut self, key: &Key) -> UiEventResult {
        let result = self.inherited_keymap.handle_key(key, |inheritance| {
            matches!(
                inheritance,
                KeymapInheritance::Focus | KeymapInheritance::Application
            )
        });
        match result {
            editor::HandleKeyResult::Complete(intent) => {
                self.key_sequence = PluginWindowKeySequence::None;
                UiEventResult::Handled(vec![intent])
            }
            editor::HandleKeyResult::WaitForMore => {
                self.key_sequence = PluginWindowKeySequence::Inherited;
                UiEventResult::Handled(Vec::new())
            }
            editor::HandleKeyResult::InvalidSequence => {
                self.key_sequence = PluginWindowKeySequence::None;
                UiEventResult::Handled(Vec::new())
            }
        }
    }

    /// Renders all visible plugin windows into the supplied UI rectangle.
    pub fn render(&mut self, screen: &mut Screen, rect: UiRect) {
        let focused = self.focused;
        let mut ids: Vec<_> = self
            .windows
            .iter()
            .filter_map(|(id, window)| window.is_visible().then_some(*id))
            .collect();
        ids.sort_unstable();
        if let Some(focused) = focused {
            ids.retain(|id| *id != focused);
            if self
                .windows
                .get(&focused)
                .is_some_and(PluginWindow::is_visible)
            {
                ids.push(focused);
            }
        }

        for id in ids {
            if let Some(window) = self.windows.get_mut(&id) {
                window.render_in_rect(screen, rect, Some(id) == focused);
            }
        }
    }
}

impl Widget for PluginWindow {
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        let Some(frame) = FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            self.options.rows,
            self.options.cols,
            self.options.placement,
        ) else {
            return;
        };
        self.render_frame(screen, frame, false);
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

/// Parses an anchor name from the plugin API.
pub fn parse_anchor(value: &str) -> Result<FloatingAnchor, String> {
    match value {
        "center" => Ok(FloatingAnchor::Center),
        "top_center" | "top-center" => Ok(FloatingAnchor::TopCenter),
        "top_right" | "top-right" => Ok(FloatingAnchor::TopRight),
        "bottom_right" | "bottom-right" => Ok(FloatingAnchor::BottomRight),
        other => Err(format!("unknown plugin window anchor {other}")),
    }
}

/// Converts a plugin window ID to the numeric script representation.
pub fn id_to_number(id: PluginWindowId) -> f64 {
    id.0 as f64
}

/// Parses a numeric script value into a plugin window ID.
pub fn id_from_number(value: f64) -> Result<PluginWindowId, String> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err("plugin window_id must be a non-negative integer".to_string());
    }
    Ok(PluginWindowId(value as usize))
}

/// Parses a canonical key sequence for a plugin window binding.
pub fn parse_key_sequence(value: &str) -> Result<Vec<String>, String> {
    editor::validate_key_string(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, KeymapsConfig};
    use crate::ui::Command;
    use crate::window::{Position, Size};
    use urvim_terminal::{Color, KeyCode, Modifiers, Style};
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn window_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(1)).bg(Color::ansi(2));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.window").unwrap(),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.border").unwrap(),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.resize").unwrap(),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)).bold(),
        );
        highlights.insert(
            Tag::parse("ui.picker.accent").unwrap(),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        );
        Theme::new("window", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn plugin_window_focus_changes_border_and_title_style() {
        let theme = window_theme();
        let border_style = theme.resolve_name_with_default("ui.window.lines.border");
        let active_style = theme.resolve_name_with_default("ui.picker.accent");
        let _theme_guard = globals::set_test_active_theme(theme);
        let mut manager = PluginWindowManager::new();
        let id = manager.create(
            "demo".to_string(),
            PluginWindowOptions {
                rows: 2,
                cols: 8,
                title: Some("Demo".to_string()),
                focused_border_style: Tag::parse("ui.picker.accent").unwrap(),
                ..PluginWindowOptions::default()
            },
        );
        let rect = UiRect::new(Position::new(0, 0), Size::new(8, 20));
        let frame = FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            2,
            8,
            PluginWindowOptions::default().placement,
        )
        .unwrap();
        let mut screen = Screen::new(8, 20);

        manager.render(&mut screen, rect);
        assert_eq!(
            screen
                .get_cell_mut(frame.origin.row, frame.origin.col)
                .unwrap()
                .style,
            border_style
        );
        assert_eq!(
            screen
                .get_cell_mut(frame.origin.row, frame.origin.col + 3)
                .unwrap()
                .style,
            border_style
        );

        manager.focus("demo", id).unwrap();
        manager.render(&mut screen, rect);
        assert_eq!(
            screen
                .get_cell_mut(frame.origin.row, frame.origin.col)
                .unwrap()
                .style,
            active_style
        );
        assert_eq!(
            screen
                .get_cell_mut(frame.origin.row, frame.origin.col + 3)
                .unwrap()
                .style,
            active_style
        );

        manager.blur_focused();
        manager.render(&mut screen, rect);
        assert_eq!(
            screen
                .get_cell_mut(frame.origin.row, frame.origin.col)
                .unwrap()
                .style,
            border_style
        );
    }

    #[test]
    fn plugin_window_lifecycle_enforces_ownership_and_focus() {
        let mut manager = PluginWindowManager::new();
        let id = manager.create("demo".to_string(), PluginWindowOptions::default());

        assert_eq!(manager.ids().collect::<Vec<_>>(), vec![id]);
        assert!(manager.owned_window("other", id).is_err());
        manager
            .focus("demo", id)
            .expect("owner should focus window");
        assert_eq!(manager.focused(), Some(id));
        manager.hide("demo", id).expect("owner should hide window");
        assert_eq!(manager.focused(), None);
        manager
            .close("demo", id)
            .expect("owner should close window");
        assert!(manager.ids().next().is_none());
    }

    #[test]
    fn focused_plugin_window_routes_keymap_intents_and_blurs_on_escape() {
        let mut manager = PluginWindowManager::new();
        let id = manager.create("demo".to_string(), PluginWindowOptions::default());
        manager
            .set_keymap(
                "demo",
                id,
                vec!["x".to_string()],
                "pane wrap-toggle".to_string(),
                Intent::Command(Command::ToggleWrap),
            )
            .expect("keymap should be accepted");
        manager.focus("demo", id).expect("window should focus");

        let key = Key::new(KeyCode::Char('x'));
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(key)),
            UiEventResult::Handled(vec![Intent::Command(Command::ToggleWrap)])
        );
        assert_eq!(manager.focused(), Some(id));

        let escape = Key {
            code: KeyCode::Esc,
            modifiers: Modifiers::default(),
        };
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(escape)),
            UiEventResult::Handled(Vec::new())
        );
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn focused_plugin_window_inherits_global_focus_cycle_keys() {
        let mut manager = PluginWindowManager::new();
        let first = manager.create("demo".to_string(), PluginWindowOptions::default());
        manager.create("demo".to_string(), PluginWindowOptions::default());
        manager.focus("demo", first).expect("window should focus");

        let control_w = Key::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL);
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(control_w)),
            UiEventResult::Handled(Vec::new())
        );
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Char('n')))),
            UiEventResult::Handled(vec![Intent::Command(Command::FocusNextWindow)])
        );
        assert_eq!(manager.focused(), Some(first));
    }

    #[test]
    fn focused_plugin_window_inherits_rebound_focus_and_application_mappings() {
        let _config_guard = globals::set_test_config(Config {
            keymaps: KeymapsConfig {
                normal: BTreeMap::from([
                    ("<C-h>".to_string(), "pane focus-left".to_string()),
                    ("<F7>".to_string(), "try-quit".to_string()),
                ]),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut manager = PluginWindowManager::new();
        let id = manager.create("demo".to_string(), PluginWindowOptions::default());
        manager.focus("demo", id).expect("window should focus");

        let focus = Key::with_modifiers(KeyCode::Char('h'), Modifiers::CTRL);
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(focus)),
            UiEventResult::Handled(vec![Intent::Command(Command::FocusPaneLeft)])
        );
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(Key::new(KeyCode::F7))),
            UiEventResult::Handled(vec![Intent::Command(Command::TryQuit)])
        );
    }

    #[test]
    fn focused_plugin_window_local_mapping_wins_over_inherited_mapping() {
        let _config_guard = globals::set_test_config(Config {
            keymaps: KeymapsConfig {
                normal: BTreeMap::from([("<F7>".to_string(), "try-quit".to_string())]),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut manager = PluginWindowManager::new();
        let id = manager.create("demo".to_string(), PluginWindowOptions::default());
        manager
            .set_keymap(
                "demo",
                id,
                vec!["<F7>".to_string()],
                "pane wrap-toggle".to_string(),
                Intent::Command(Command::ToggleWrap),
            )
            .expect("local mapping should be accepted");
        manager.focus("demo", id).expect("window should focus");

        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(Key::new(KeyCode::F7))),
            UiEventResult::Handled(vec![Intent::Command(Command::ToggleWrap)])
        );
    }

    #[test]
    fn plugin_window_content_rejects_newline_only_at_host_boundary() {
        let content = vec![vec![PluginWindowSegment {
            text: "line".to_string(),
            style: None,
        }]];
        let mut manager = PluginWindowManager::new();
        let id = manager.create("demo".to_string(), PluginWindowOptions::default());
        manager
            .set_content("demo", id, content)
            .expect("core manager stores retained content");
        assert_eq!(manager.owned_window("demo", id).unwrap().content().len(), 1);
    }

    #[test]
    fn plugin_window_margins_apply_to_every_anchor() {
        let margins = FloatingMargins {
            top: 2,
            right: 3,
            bottom: 4,
            left: 5,
        };
        let origin = Position::new(10, 20);
        let size = Size::new(40, 100);

        let frame = |anchor| {
            FloatingWindowFrame::resolve_placement(
                origin,
                size,
                8,
                20,
                FloatingPlacement::Anchored { anchor, margins },
            )
            .expect("window should fit inside the inset bounds")
        };

        assert_eq!(frame(FloatingAnchor::Center).origin, Position::new(24, 60));
        assert_eq!(
            frame(FloatingAnchor::TopCenter).origin,
            Position::new(12, 60)
        );
        assert_eq!(
            frame(FloatingAnchor::TopRight).origin,
            Position::new(12, 95)
        );
        assert_eq!(
            frame(FloatingAnchor::BottomRight).origin,
            Position::new(36, 95)
        );
    }

    #[test]
    fn plugin_window_margins_clip_to_inset_bounds() {
        let options = PluginWindowOptions {
            rows: 8,
            cols: 40,
            placement: FloatingPlacement::Anchored {
                anchor: FloatingAnchor::Center,
                margins: FloatingMargins {
                    top: 5,
                    right: 5,
                    bottom: 5,
                    left: 5,
                },
            },
            ..PluginWindowOptions::default()
        };
        let frame = FloatingWindowFrame::resolve_placement(
            Position::new(0, 0),
            Size::new(20, 30),
            options.rows,
            options.cols,
            options.placement,
        )
        .expect("window should be clipped to the inset bounds");
        assert_eq!(frame.origin, Position::new(5, 5));
        assert_eq!(frame.size, Size::new(10, 20));

        let impossible = PluginWindowOptions {
            placement: FloatingPlacement::Anchored {
                anchor: FloatingAnchor::Center,
                margins: FloatingMargins {
                    top: 5,
                    bottom: 5,
                    ..FloatingMargins::default()
                },
            },
            ..PluginWindowOptions::default()
        };
        assert!(
            FloatingWindowFrame::resolve_placement(
                Position::new(0, 0),
                Size::new(10, 20),
                impossible.rows,
                impossible.cols,
                impossible.placement,
            )
            .is_none()
        );
    }

    #[test]
    fn plugin_window_fixed_position_preserves_origin_and_clips_size() {
        let options = PluginWindowOptions {
            placement: FloatingPlacement::Fixed { row: 4, col: 7 },
            rows: 8,
            cols: 40,
            ..PluginWindowOptions::default()
        };
        let frame = FloatingWindowFrame::resolve_placement(
            Position::new(10, 20),
            Size::new(20, 30),
            options.rows,
            options.cols,
            options.placement,
        )
        .expect("fixed window should fit at least a bordered frame");

        assert_eq!(frame.origin, Position::new(14, 27));
        assert_eq!(frame.size, Size::new(10, 23));
        assert_eq!(frame.content_size, Size::new(8, 21));
    }

    #[test]
    fn plugin_window_fixed_position_rejects_origins_without_frame_space() {
        let options = PluginWindowOptions {
            placement: FloatingPlacement::Fixed { row: 19, col: 0 },
            ..PluginWindowOptions::default()
        };
        assert!(
            FloatingWindowFrame::resolve_placement(
                Position::new(0, 0),
                Size::new(20, 40),
                options.rows,
                options.cols,
                options.placement,
            )
            .is_none()
        );

        let options = PluginWindowOptions {
            placement: FloatingPlacement::Fixed { row: 0, col: 39 },
            ..PluginWindowOptions::default()
        };
        assert!(
            FloatingWindowFrame::resolve_placement(
                Position::new(0, 0),
                Size::new(20, 40),
                options.rows,
                options.cols,
                options.placement,
            )
            .is_none()
        );
    }
}
