//! Retained overlays owned by plugins.

pub mod frame;

use crate::editor::{InheritedKeymap, NormalMode, TrieKeymap};
use crate::screen::Screen;
use crate::ui::overlay::frame::{
    OverlayAnchor, OverlayFrame, OverlayFrameLabel, OverlayMargins, OverlayPlacement,
};
use crate::ui::{
    FocusPolicy, Intent, KeymapInheritance, UiContext, UiEvent, UiEventResult, UiRect,
};
use crate::widget::Widget;
use crate::{editor, globals};
use std::collections::BTreeMap;
use urvim_terminal::Key;
use urvim_theme::Tag;

/// Stable identifier for a plugin-owned overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlayId(pub usize);

/// Configuration for a plugin overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOptions {
    /// Placement mode.
    pub placement: OverlayPlacement,
    /// Content height in terminal rows.
    pub rows: u16,
    /// Content width in terminal columns.
    pub cols: u16,
    /// Optional border title.
    pub title: Option<String>,
    /// Theme tag for the overlay body.
    pub body_style: Tag,
    /// Theme tag for the unfocused overlay border.
    pub border_style: Tag,
    /// Theme tag for the focused overlay border.
    pub focused_border_style: Tag,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            placement: OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins::default(),
            },
            rows: 8,
            cols: 40,
            title: None,
            body_style: Tag::parse("ui.window").expect("built-in theme tag should parse"),
            border_style: Tag::parse("ui.window.lines.border")
                .expect("built-in border tag should parse"),
            focused_border_style: Tag::parse("ui.window.lines.resize")
                .expect("built-in focused border tag should parse"),
        }
    }
}

/// A styled text segment in retained plugin UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedSegment {
    /// Segment text without a newline.
    pub text: String,
    /// Optional theme tag overlaid on the surface body style.
    pub style: Option<Tag>,
}

/// Retained styled content shared by plugin panes and overlays.
pub type RetainedContent = Vec<Vec<RetainedSegment>>;

#[derive(Debug, Clone)]
struct KeyBinding {
    rhs: String,
    intent: Intent,
}

/// Shared retained content and local keymap state for plugin UI surfaces.
#[derive(Debug)]
pub struct RetainedSurface {
    owner: String,
    content: RetainedContent,
    keymaps: TrieKeymap<KeyBinding>,
    pending_keys: Vec<String>,
}

impl RetainedSurface {
    /// Creates an empty retained surface owned by a plugin.
    pub fn new(owner: String) -> Self {
        Self {
            owner,
            content: Vec::new(),
            keymaps: TrieKeymap::new(),
            pending_keys: Vec::new(),
        }
    }

    /// Returns the owning plugin name.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Returns the retained content.
    pub fn content(&self) -> &RetainedContent {
        &self.content
    }

    /// Replaces the retained content.
    pub fn set_content(&mut self, content: RetainedContent) {
        self.content = content;
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

    /// Renders retained content across a rectangle with the given body theme tag.
    pub fn render(&self, screen: &mut Screen, rect: UiRect, body_tag: &Tag) {
        if rect.size.rows == 0 || rect.size.cols == 0 {
            return;
        }
        let body_style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default(body_tag.as_str()))
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
                let clipped = crate::ui::text_width::clip_text(
                    segment.text.as_str(),
                    usize::from(right_col - col),
                    crate::ui::text_width::ClipSide::Start,
                );
                screen.write_string(row, col, body_style.overlay(style), clipped.text.as_str());
                col = col.saturating_add(clipped.width as u16);
            }
        }
    }

    fn has_explicit_escape_binding(&self) -> bool {
        self.keymaps.is_prefix(&["<Esc>".to_string()])
    }
}

/// A plugin-owned overlay and its retained UI state.
#[derive(Debug)]
pub struct Overlay {
    surface: RetainedSurface,
    options: OverlayOptions,
    visible: bool,
}

impl Overlay {
    /// Creates a retained plugin-owned overlay.
    pub fn new(owner: String, options: OverlayOptions) -> Self {
        Self {
            surface: RetainedSurface::new(owner),
            options,
            visible: true,
        }
    }

    /// Returns the plugin that owns this overlay.
    pub fn owner(&self) -> &str {
        self.surface.owner()
    }

    /// Returns the current overlay configuration.
    pub fn options(&self) -> &OverlayOptions {
        &self.options
    }

    /// Returns the current retained content.
    pub fn content(&self) -> &RetainedContent {
        self.surface.content()
    }

    /// Returns whether this overlay is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Updates the overlay's presentation options.
    pub fn set_options(&mut self, options: OverlayOptions) {
        self.options = options;
    }

    /// Replaces the retained overlay content.
    pub fn set_content(&mut self, content: RetainedContent) {
        self.surface.set_content(content);
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Installs a local keymap binding.
    pub fn set_keymap(&mut self, keys: Vec<String>, rhs: String, intent: Intent) {
        self.surface.set_keymap(keys, rhs, intent);
    }

    /// Removes a local keymap binding.
    pub fn delete_keymap(&mut self, keys: &[String]) {
        self.surface.delete_keymap(keys);
    }

    /// Returns all local keymap bindings.
    pub fn keymaps(&self) -> Vec<(Vec<String>, String)> {
        self.surface.keymaps()
    }

    /// Routes a key through the local keymap.
    pub fn handle_key(&mut self, key: &Key) -> editor::HandleKeyResult {
        self.surface.handle_key(key)
    }

    /// Clears any partially entered local key sequence.
    pub fn clear_pending_keys(&mut self) {
        self.surface.clear_pending_keys();
    }

    fn has_explicit_escape_binding(&self) -> bool {
        self.surface.has_explicit_escape_binding()
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
            .surface
            .content()
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
        let Some(frame) = OverlayFrame::resolve_placement(
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

    fn render_frame(&mut self, screen: &mut Screen, frame: OverlayFrame, focused: bool) {
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
                .map(OverlayFrameLabel::top_center),
        );

        self.render_content_in_rect(
            screen,
            UiRect::new(frame.content_origin, frame.content_size),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayKeySequence {
    None,
    Local,
    Inherited,
}

/// Retained registry of plugin overlays.
#[derive(Debug)]
pub struct OverlayManager {
    overlays: BTreeMap<OverlayId, Overlay>,
    next_id: usize,
    focused: Option<OverlayId>,
    inherited_keymap: InheritedKeymap,
    key_sequence: OverlayKeySequence,
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self {
            overlays: BTreeMap::new(),
            next_id: 0,
            focused: None,
            inherited_keymap: InheritedKeymap::new(NormalMode::keymap()),
            key_sequence: OverlayKeySequence::None,
        }
    }
}

impl OverlayManager {
    /// Creates an empty overlay registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all currently registered overlay IDs.
    pub fn ids(&self) -> impl Iterator<Item = OverlayId> + '_ {
        self.overlays.keys().copied()
    }

    /// Returns visible overlay IDs in stable creation order.
    pub fn visible_ids(&self) -> impl Iterator<Item = OverlayId> + '_ {
        self.overlays
            .iter()
            .filter_map(|(id, overlay)| overlay.is_visible().then_some(*id))
    }

    /// Returns the currently focused overlay, if any.
    pub fn focused(&self) -> Option<OverlayId> {
        self.focused
    }

    /// Creates a visible, unfocused overlay.
    pub fn create(&mut self, owner: String, options: OverlayOptions) -> OverlayId {
        let id = OverlayId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.overlays.insert(id, Overlay::new(owner, options));
        id
    }

    /// Returns an overlay owned by `owner` or an ownership error.
    pub fn owned_overlay(&self, owner: &str, id: OverlayId) -> Result<&Overlay, String> {
        let overlay = self
            .overlays
            .get(&id)
            .ok_or_else(|| format!("unknown overlay_id {}", id.0))?;
        if overlay.owner() != owner {
            return Err(format!("plugin {owner:?} does not own overlay_id {}", id.0));
        }
        Ok(overlay)
    }

    /// Returns a mutable overlay owned by `owner` or an ownership error.
    pub fn owned_overlay_mut(
        &mut self,
        owner: &str,
        id: OverlayId,
    ) -> Result<&mut Overlay, String> {
        let overlay = self
            .overlays
            .get_mut(&id)
            .ok_or_else(|| format!("unknown overlay_id {}", id.0))?;
        if overlay.owner() != owner {
            return Err(format!("plugin {owner:?} does not own overlay_id {}", id.0));
        }
        Ok(overlay)
    }

    /// Updates an overlay's configuration.
    pub fn configure(
        &mut self,
        owner: &str,
        id: OverlayId,
        options: OverlayOptions,
    ) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?.set_options(options);
        Ok(())
    }

    /// Replaces an overlay's retained content.
    pub fn set_content(
        &mut self,
        owner: &str,
        id: OverlayId,
        content: RetainedContent,
    ) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?.set_content(content);
        Ok(())
    }

    /// Shows an overlay.
    pub fn show(&mut self, owner: &str, id: OverlayId) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?.set_visible(true);
        Ok(())
    }

    /// Hides an overlay and clears focus if necessary.
    pub fn hide(&mut self, owner: &str, id: OverlayId) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?.set_visible(false);
        if self.focused == Some(id) {
            self.blur_focused();
        }
        Ok(())
    }

    /// Focuses a visible overlay.
    pub fn focus(&mut self, owner: &str, id: OverlayId) -> Result<(), String> {
        let overlay = self.owned_overlay(owner, id)?;
        if !overlay.is_visible() {
            return Err(format!("overlay_id {} is hidden", id.0));
        }
        self.focus_id(id);
        Ok(())
    }

    /// Focuses a visible overlay by ID for layout-level focus traversal.
    pub fn focus_id(&mut self, id: OverlayId) -> bool {
        let Some(overlay) = self.overlays.get(&id) else {
            return false;
        };
        if !overlay.is_visible() {
            return false;
        }
        self.clear_pending_keys();
        self.focused = Some(id);
        if let Some(overlay) = self.overlays.get_mut(&id) {
            overlay.clear_pending_keys();
        }
        true
    }

    /// Clears overlay focus without requiring an owning plugin.
    pub fn blur_focused(&mut self) {
        self.clear_pending_keys();
        self.focused = None;
    }

    /// Clears overlay focus.
    pub fn blur(&mut self, owner: &str, id: OverlayId) -> Result<(), String> {
        self.owned_overlay(owner, id)?;
        if self.focused == Some(id) {
            self.blur_focused();
        }
        Ok(())
    }

    /// Closes an overlay and clears focus if necessary.
    pub fn close(&mut self, owner: &str, id: OverlayId) -> Result<(), String> {
        self.owned_overlay(owner, id)?;
        if self.focused == Some(id) {
            self.blur_focused();
        }
        self.overlays.remove(&id);
        Ok(())
    }

    /// Closes all overlays owned by a plugin.
    pub fn close_owned(&mut self, owner: &str) {
        let ids: Vec<_> = self
            .overlays
            .iter()
            .filter_map(|(id, overlay)| (overlay.owner() == owner).then_some(*id))
            .collect();
        for id in ids {
            if self.focused == Some(id) {
                self.blur_focused();
            }
            self.overlays.remove(&id);
        }
    }

    /// Installs a command binding for an overlay.
    pub fn set_keymap(
        &mut self,
        owner: &str,
        id: OverlayId,
        keys: Vec<String>,
        rhs: String,
        intent: Intent,
    ) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?
            .set_keymap(keys, rhs, intent);
        if self.focused == Some(id) {
            self.clear_pending_keys();
        }
        Ok(())
    }

    /// Removes a command binding from an overlay.
    pub fn delete_keymap(
        &mut self,
        owner: &str,
        id: OverlayId,
        keys: &[String],
    ) -> Result<(), String> {
        self.owned_overlay_mut(owner, id)?.delete_keymap(keys);
        if self.focused == Some(id) {
            self.clear_pending_keys();
        }
        Ok(())
    }

    /// Returns an overlay's configured keymaps.
    pub fn keymaps(
        &self,
        owner: &str,
        id: OverlayId,
    ) -> Result<Vec<(Vec<String>, String)>, String> {
        let overlay = self.owned_overlay(owner, id)?;
        Ok(overlay.keymaps())
    }

    /// Routes an event to the focused overlay.
    pub fn route_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(id) = self.focused else {
            return UiEventResult::NotHandled;
        };

        if !self.overlays.contains_key(&id) {
            self.focused = None;
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => {
                // An inherited prefix owns the rest of its sequence; otherwise
                // Overlay-local mappings always receive the key first.
                if self.key_sequence == OverlayKeySequence::Inherited {
                    return self.route_inherited_key(key);
                }

                let Some(overlay) = self.overlays.get_mut(&id) else {
                    self.focused = None;
                    return UiEventResult::NotHandled;
                };
                if key.canonical_string() == "<Esc>" && !overlay.has_explicit_escape_binding() {
                    overlay.clear_pending_keys();
                    self.blur_focused();
                    return UiEventResult::Handled(Vec::new());
                }

                match overlay.handle_key(key) {
                    editor::HandleKeyResult::Complete(intent) => {
                        self.key_sequence = OverlayKeySequence::None;
                        UiEventResult::Handled(vec![intent])
                    }
                    editor::HandleKeyResult::WaitForMore => {
                        self.key_sequence = OverlayKeySequence::Local;
                        UiEventResult::Handled(Vec::new())
                    }
                    editor::HandleKeyResult::InvalidSequence
                        if self.key_sequence == OverlayKeySequence::Local =>
                    {
                        self.key_sequence = OverlayKeySequence::None;
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
        self.key_sequence = OverlayKeySequence::None;
        if let Some(overlay) = self.focused.and_then(|id| self.overlays.get_mut(&id)) {
            overlay.clear_pending_keys();
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
                self.key_sequence = OverlayKeySequence::None;
                UiEventResult::Handled(vec![intent])
            }
            editor::HandleKeyResult::WaitForMore => {
                self.key_sequence = OverlayKeySequence::Inherited;
                UiEventResult::Handled(Vec::new())
            }
            editor::HandleKeyResult::InvalidSequence => {
                self.key_sequence = OverlayKeySequence::None;
                UiEventResult::Handled(Vec::new())
            }
        }
    }

    /// Renders all visible overlays into the supplied UI rectangle.
    pub fn render(&mut self, screen: &mut Screen, rect: UiRect) {
        let focused = self.focused;
        let mut ids: Vec<_> = self
            .overlays
            .iter()
            .filter_map(|(id, overlay)| overlay.is_visible().then_some(*id))
            .collect();
        ids.sort_unstable();
        if let Some(focused) = focused {
            ids.retain(|id| *id != focused);
            if self.overlays.get(&focused).is_some_and(Overlay::is_visible) {
                ids.push(focused);
            }
        }

        for id in ids {
            if let Some(overlay) = self.overlays.get_mut(&id) {
                overlay.render_in_rect(screen, rect, Some(id) == focused);
            }
        }
    }
}

impl Widget for Overlay {
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        let Some(frame) = OverlayFrame::resolve_placement(
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
pub fn parse_anchor(value: &str) -> Result<OverlayAnchor, String> {
    match value {
        "center" => Ok(OverlayAnchor::Center),
        "top_center" | "top-center" => Ok(OverlayAnchor::TopCenter),
        "top_right" | "top-right" => Ok(OverlayAnchor::TopRight),
        "bottom_right" | "bottom-right" => Ok(OverlayAnchor::BottomRight),
        other => Err(format!("unknown overlay anchor {other}")),
    }
}

/// Converts an overlay ID to the numeric script representation.
pub fn id_to_number(id: OverlayId) -> f64 {
    id.0 as f64
}

/// Parses a numeric script value into an overlay ID.
pub fn id_from_number(value: f64) -> Result<OverlayId, String> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err("overlay_id must be a non-negative integer".to_string());
    }
    Ok(OverlayId(value as usize))
}

/// Parses a canonical key sequence for an overlay binding.
pub fn parse_key_sequence(value: &str) -> Result<Vec<String>, String> {
    editor::validate_key_string(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, KeymapsConfig};
    use crate::ui::Command;
    use crate::ui::geometry::{Position, Size};
    use urvim_terminal::{Color, KeyCode, Modifiers, Style};
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn overlay_theme() -> Theme {
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
        Theme::new("overlay", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn overlay_focus_changes_border_and_title_style() {
        let theme = overlay_theme();
        let border_style = theme.resolve_name_with_default("ui.window.lines.border");
        let active_style = theme.resolve_name_with_default("ui.picker.accent");
        let _theme_guard = globals::set_test_active_theme(theme);
        let mut manager = OverlayManager::new();
        let id = manager.create(
            "demo".to_string(),
            OverlayOptions {
                rows: 2,
                cols: 8,
                title: Some("Demo".to_string()),
                focused_border_style: Tag::parse("ui.picker.accent").unwrap(),
                ..OverlayOptions::default()
            },
        );
        let rect = UiRect::new(Position::new(0, 0), Size::new(8, 20));
        let frame = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            2,
            8,
            OverlayOptions::default().placement,
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
    fn overlay_lifecycle_enforces_ownership_and_focus() {
        let mut manager = OverlayManager::new();
        let id = manager.create("demo".to_string(), OverlayOptions::default());

        assert_eq!(manager.ids().collect::<Vec<_>>(), vec![id]);
        assert!(manager.owned_overlay("other", id).is_err());
        manager
            .focus("demo", id)
            .expect("owner should focus overlay");
        assert_eq!(manager.focused(), Some(id));
        manager.hide("demo", id).expect("owner should hide overlay");
        assert_eq!(manager.focused(), None);
        manager
            .close("demo", id)
            .expect("owner should close overlay");
        assert!(manager.ids().next().is_none());
    }

    #[test]
    fn focused_overlay_routes_keymap_intents_and_blurs_on_escape() {
        let mut manager = OverlayManager::new();
        let id = manager.create("demo".to_string(), OverlayOptions::default());
        manager
            .set_keymap(
                "demo",
                id,
                vec!["x".to_string()],
                "pane wrap-toggle".to_string(),
                Intent::Command(Command::ToggleWrap),
            )
            .expect("keymap should be accepted");
        manager.focus("demo", id).expect("overlay should focus");

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
    fn overlay_focus_changes_clear_previous_local_key_sequences() {
        let mut manager = OverlayManager::new();
        let first = manager.create("demo".to_string(), OverlayOptions::default());
        let second = manager.create("demo".to_string(), OverlayOptions::default());
        manager
            .set_keymap(
                "demo",
                first,
                vec!["g".to_string(), "g".to_string()],
                "pane wrap-toggle".to_string(),
                Intent::Command(Command::ToggleWrap),
            )
            .unwrap();
        let g = Key::new(KeyCode::Char('g'));

        manager.focus("demo", first).unwrap();
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(g)),
            UiEventResult::Handled(Vec::new())
        );
        manager.focus("demo", second).unwrap();
        assert!(manager.overlays[&first].surface.pending_keys.is_empty());

        manager.focus("demo", first).unwrap();
        manager.route_ui_event(&UiEvent::Key(g));
        manager.blur_focused();
        assert!(manager.overlays[&first].surface.pending_keys.is_empty());

        manager.focus("demo", first).unwrap();
        manager.route_ui_event(&UiEvent::Key(g));
        manager.hide("demo", first).unwrap();
        assert!(manager.overlays[&first].surface.pending_keys.is_empty());
    }

    #[test]
    fn focused_overlay_inherits_global_focus_cycle_keys() {
        let mut manager = OverlayManager::new();
        let first = manager.create("demo".to_string(), OverlayOptions::default());
        manager.create("demo".to_string(), OverlayOptions::default());
        manager.focus("demo", first).expect("overlay should focus");

        let control_w = Key::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL);
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(control_w)),
            UiEventResult::Handled(Vec::new())
        );
        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Char('n')))),
            UiEventResult::Handled(vec![Intent::Command(Command::FocusNextTarget)])
        );
        assert_eq!(manager.focused(), Some(first));
    }

    #[test]
    fn focused_overlay_inherits_rebound_focus_and_application_mappings() {
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
        let mut manager = OverlayManager::new();
        let id = manager.create("demo".to_string(), OverlayOptions::default());
        manager.focus("demo", id).expect("overlay should focus");

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
    fn focused_overlay_local_mapping_wins_over_inherited_mapping() {
        let _config_guard = globals::set_test_config(Config {
            keymaps: KeymapsConfig {
                normal: BTreeMap::from([("<F7>".to_string(), "try-quit".to_string())]),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut manager = OverlayManager::new();
        let id = manager.create("demo".to_string(), OverlayOptions::default());
        manager
            .set_keymap(
                "demo",
                id,
                vec!["<F7>".to_string()],
                "pane wrap-toggle".to_string(),
                Intent::Command(Command::ToggleWrap),
            )
            .expect("local mapping should be accepted");
        manager.focus("demo", id).expect("overlay should focus");

        assert_eq!(
            manager.route_ui_event(&UiEvent::Key(Key::new(KeyCode::F7))),
            UiEventResult::Handled(vec![Intent::Command(Command::ToggleWrap)])
        );
    }

    #[test]
    fn overlay_content_rejects_newline_only_at_host_boundary() {
        let content = vec![vec![RetainedSegment {
            text: "line".to_string(),
            style: None,
        }]];
        let mut manager = OverlayManager::new();
        let id = manager.create("demo".to_string(), OverlayOptions::default());
        manager
            .set_content("demo", id, content)
            .expect("core manager stores retained content");
        assert_eq!(
            manager.owned_overlay("demo", id).unwrap().content().len(),
            1
        );
    }

    #[test]
    fn overlay_margins_apply_to_every_anchor() {
        let margins = OverlayMargins {
            top: 2,
            right: 3,
            bottom: 4,
            left: 5,
        };
        let origin = Position::new(10, 20);
        let size = Size::new(40, 100);

        let frame = |anchor| {
            OverlayFrame::resolve_placement(
                origin,
                size,
                8,
                20,
                OverlayPlacement::Anchored { anchor, margins },
            )
            .expect("overlay should fit inside the inset bounds")
        };

        assert_eq!(frame(OverlayAnchor::Center).origin, Position::new(24, 60));
        assert_eq!(
            frame(OverlayAnchor::TopCenter).origin,
            Position::new(12, 60)
        );
        assert_eq!(frame(OverlayAnchor::TopRight).origin, Position::new(12, 95));
        assert_eq!(
            frame(OverlayAnchor::BottomRight).origin,
            Position::new(36, 95)
        );
    }

    #[test]
    fn overlay_margins_clip_to_inset_bounds() {
        let options = OverlayOptions {
            rows: 8,
            cols: 40,
            placement: OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins {
                    top: 5,
                    right: 5,
                    bottom: 5,
                    left: 5,
                },
            },
            ..OverlayOptions::default()
        };
        let frame = OverlayFrame::resolve_placement(
            Position::new(0, 0),
            Size::new(20, 30),
            options.rows,
            options.cols,
            options.placement,
        )
        .expect("overlay should be clipped to the inset bounds");
        assert_eq!(frame.origin, Position::new(5, 5));
        assert_eq!(frame.size, Size::new(10, 20));

        let impossible = OverlayOptions {
            placement: OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins {
                    top: 5,
                    bottom: 5,
                    ..OverlayMargins::default()
                },
            },
            ..OverlayOptions::default()
        };
        assert!(
            OverlayFrame::resolve_placement(
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
    fn overlay_fixed_position_preserves_origin_and_clips_size() {
        let options = OverlayOptions {
            placement: OverlayPlacement::Fixed { row: 4, col: 7 },
            rows: 8,
            cols: 40,
            ..OverlayOptions::default()
        };
        let frame = OverlayFrame::resolve_placement(
            Position::new(10, 20),
            Size::new(20, 30),
            options.rows,
            options.cols,
            options.placement,
        )
        .expect("fixed overlay should fit at least a bordered frame");

        assert_eq!(frame.origin, Position::new(14, 27));
        assert_eq!(frame.size, Size::new(10, 23));
        assert_eq!(frame.content_size, Size::new(8, 21));
    }

    #[test]
    fn overlay_fixed_position_rejects_origins_without_frame_space() {
        let options = OverlayOptions {
            placement: OverlayPlacement::Fixed { row: 19, col: 0 },
            ..OverlayOptions::default()
        };
        assert!(
            OverlayFrame::resolve_placement(
                Position::new(0, 0),
                Size::new(20, 40),
                options.rows,
                options.cols,
                options.placement,
            )
            .is_none()
        );

        let options = OverlayOptions {
            placement: OverlayPlacement::Fixed { row: 0, col: 39 },
            ..OverlayOptions::default()
        };
        assert!(
            OverlayFrame::resolve_placement(
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
