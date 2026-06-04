use super::*;
use crate::action::ActionResult;
use crate::background::{BackgroundJob, JobKind, JobToken};
use crate::buffer::{BufferId, Cursor, DiffHunk, DiffMarkerKind, DiffRefreshResult, TextRef};
use crate::config::{
    AdvancedGlyphCapability, AutoIndentMode, Config, DefaultRegisters, ScrollMargin, WrapMode,
};
use crate::editor::{
    Action, ActionKind, BoundaryMotion, BracketKind, DelimiterFamily, LinewiseMotion, ModeKind,
};
use crate::editor::{Operator, OperatorTarget, QuoteKind, TextObject};
use crate::globals;
use crate::globals::{Direction, FindKind, FindState};
use crate::lsp::diagnostics::{diagnostic_style_for, diagnostic_undercurl_style_for};
use crate::path::AbsolutePath;
use crate::register::{RegisterContent, RegisterContentKind, RegisterName, RegisterStore};
use crate::terminal::{Color, Style};
use crate::theme::{HighlightStyles, Tag, Theme, ThemeKind};
use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tracing_subscriber::layer::SubscriberExt;

const CONTENT_COL: u16 = 5;

fn process_action_and_snapshot(window: &mut Window, action: &Action) {
    assert_eq!(window.dispatch_action(action), ActionResult::Handled);

    if action.is_snapshottable() {
        let cursor = window.buffer_view.cursor();
        window
            .buffer_view
            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
            .unwrap_or(());
    }
}

fn dispatch_with_main_loop_snapshot(window: &mut Window, action: &Action) {
    assert_eq!(window.dispatch_action(action), ActionResult::Handled);

    if action.is_snapshottable() {
        let cursor = window.buffer_view.cursor();
        window
            .buffer_view
            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
            .unwrap_or(());
    }
}

#[test]
fn test_surround_replace_updates_nearest_enclosing_pair() {
    let mut window = Window::new(Buffer::from_str("one {two {three} four} five"));
    window.set_cursor(Cursor::new(0, 10));

    let action = Action::new(ActionKind::SurroundReplace {
        target: DelimiterFamily::Curly,
        replacement: DelimiterFamily::Square,
    });
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(
        buffer_text(window.buffer_view()),
        "one {two [three] four} five"
    );
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.char_at_cursor(window.buffer_view().cursor()))
            .unwrap_or(None),
        Some('[')
    );
}

#[test]
fn test_surround_delete_works_across_lines() {
    let mut window = Window::new(Buffer::from_str("foo \"bar\nbaz\" qux"));
    window.set_cursor(Cursor::new(1, 1));

    let action = Action::new(ActionKind::SurroundDelete {
        target: DelimiterFamily::DoubleQuote,
    });
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo bar\nbaz qux");
}

#[test]
fn test_surround_actions_noop_when_unresolvable_or_same_family() {
    let mut missing = Window::new(Buffer::from_str("no delimiters here"));
    missing.set_cursor(Cursor::new(0, 3));
    let delete_action = Action::new(ActionKind::SurroundDelete {
        target: DelimiterFamily::Curly,
    });
    assert_eq!(
        missing.dispatch_action(&delete_action),
        ActionResult::NotHandled
    );
    assert_eq!(buffer_text(missing.buffer_view()), "no delimiters here");

    let mut same_family = Window::new(Buffer::from_str("wrap (me) please"));
    same_family.set_cursor(Cursor::new(0, 6));
    let replace_action = Action::new(ActionKind::SurroundReplace {
        target: DelimiterFamily::Paren,
        replacement: DelimiterFamily::Paren,
    });
    assert_eq!(
        same_family.dispatch_action(&replace_action),
        ActionResult::NotHandled
    );
    assert_eq!(buffer_text(same_family.buffer_view()), "wrap (me) please");
}

#[test]
fn test_surround_replace_is_single_undoable_edit() {
    let mut window = Window::new(Buffer::from_str("foo(bar)baz"));
    window.set_cursor(Cursor::new(0, 4));

    process_action_and_snapshot(
        &mut window,
        &Action::new(ActionKind::SurroundReplace {
            target: DelimiterFamily::Paren,
            replacement: DelimiterFamily::Square,
        }),
    );
    assert_eq!(buffer_text(window.buffer_view()), "foo[bar]baz");

    apply_undo_synced(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "foo(bar)baz");
}

#[test]
fn test_replace_mode_edits_undo_as_single_snapshot() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 1));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceChar('a')).with_from_mode(ModeKind::Replace),
    );
    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceChar('b')).with_from_mode(ModeKind::Replace),
    );
    commit_insert_exit_snapshot(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "hablo");
    apply_undo_synced(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "hello");
}

#[test]
fn test_replace_backspace_restores_overwritten_character() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 1));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::ReplaceChar('a')).with_from_mode(ModeKind::Replace)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hallo");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::ReplaceBackspace {
                cursor: Cursor::new(0, 1),
                replaced: Some('e'),
                inserted: 'a',
            })
            .with_from_mode(ModeKind::Replace)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 1));
}

#[test]
fn test_replace_backspace_restores_successive_live_replace_positions() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 1));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceChar('a')).with_from_mode(ModeKind::Replace),
    );
    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceChar('b')).with_from_mode(ModeKind::Replace),
    );
    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceChar('c')).with_from_mode(ModeKind::Replace),
    );
    assert_eq!(buffer_text(window.buffer_view()), "habco");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceBackspaceLast).with_from_mode(ModeKind::Replace),
    );
    assert_eq!(buffer_text(window.buffer_view()), "hablo");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 3));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceBackspaceLast).with_from_mode(ModeKind::Replace),
    );
    assert_eq!(buffer_text(window.buffer_view()), "hallo");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
}

#[test]
fn test_replace_backspace_removes_inserted_character_past_line_end() {
    let mut window = Window::new(Buffer::from_str("hi"));
    window.set_cursor(Cursor::new(0, 2));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::ReplaceChar('x')).with_from_mode(ModeKind::Replace)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hix");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::ReplaceBackspace {
                cursor: Cursor::new(0, 2),
                replaced: None,
                inserted: 'x',
            })
            .with_from_mode(ModeKind::Replace)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hi");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 1));
}

#[test]
fn test_replace_backspace_rejoins_line_split_by_replace_newline() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 2));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::insert_newline().with_from_mode(ModeKind::Replace),
    );
    assert_eq!(buffer_text(window.buffer_view()), "he\nllo");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));

    dispatch_with_main_loop_snapshot(
        &mut window,
        &Action::new(ActionKind::ReplaceBackspaceLast).with_from_mode(ModeKind::Replace),
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
}

#[test]
fn test_surround_add_inner_word_with_quotes() {
    let mut window = Window::new(Buffer::from_str("hello world"));
    window.set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::SurroundAdd {
        target: TextObject::InnerWord,
        delimiter: DelimiterFamily::DoubleQuote,
    });
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "\"hello\" world");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_surround_add_bracket_selector_result() {
    let mut window = Window::new(Buffer::from_str("hello world"));
    window.set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::SurroundAdd {
        target: TextObject::InnerWord,
        delimiter: DelimiterFamily::Paren,
    });
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "(hello) world");
}

#[test]
fn test_surround_add_noops_when_text_object_unresolvable() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 0));

    let action = Action::new(ActionKind::SurroundAdd {
        target: TextObject::InnerBracket(BracketKind::Paren),
        delimiter: DelimiterFamily::DoubleQuote,
    });
    assert_eq!(window.dispatch_action(&action), ActionResult::NotHandled);
    assert_eq!(buffer_text(window.buffer_view()), "hello");
}

#[test]
fn test_surround_add_visual_selection_with_quotes() {
    let mut window = Window::new(Buffer::from_str("foo bar baz"));
    window.buffer_view_mut().set_cursor(Cursor::new(0, 4));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 6));

    let action = Action::new(ActionKind::SurroundAddSelection {
        delimiter: DelimiterFamily::DoubleQuote,
    })
    .with_from_mode(ModeKind::Visual)
    .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo \"bar\" baz");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
}

#[test]
fn test_surround_add_visual_selection_with_square_brackets() {
    let mut window = Window::new(Buffer::from_str("foo bar baz"));
    window.buffer_view_mut().set_cursor(Cursor::new(0, 4));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 6));

    let action = Action::new(ActionKind::SurroundAddSelection {
        delimiter: DelimiterFamily::Square,
    })
    .with_from_mode(ModeKind::Visual)
    .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo [bar] baz");
}

#[test]
fn test_surround_add_visual_line_selection_without_auto_indent() {
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Off));
    let mut window = Window::new(Buffer::from_str("alpha\nbeta"));
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 0));

    let action = Action::new(ActionKind::SurroundAddSelection {
        delimiter: DelimiterFamily::Curly,
    })
    .with_from_mode(ModeKind::VisualLine)
    .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "{\nalpha\nbeta\n}");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_surround_add_visual_line_selection_with_auto_indent() {
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    let mut window = Window::new(Buffer::from_str("alpha\nbeta"));
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 0));

    let action = Action::new(ActionKind::SurroundAddSelection {
        delimiter: DelimiterFamily::Curly,
    })
    .with_from_mode(ModeKind::VisualLine)
    .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(
        buffer_text(window.buffer_view()),
        "{\n    alpha\n    beta\n}"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_surround_add_is_single_undoable_edit() {
    let mut window = Window::new(Buffer::from_str("hello world"));
    window.set_cursor(Cursor::new(0, 1));

    process_action_and_snapshot(
        &mut window,
        &Action::new(ActionKind::SurroundAdd {
            target: TextObject::InnerWord,
            delimiter: DelimiterFamily::DoubleQuote,
        }),
    );
    assert_eq!(buffer_text(window.buffer_view()), "\"hello\" world");

    apply_undo_synced(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "hello world");
}

fn commit_insert_exit_snapshot(window: &mut Window) {
    let cursor = window.buffer_view.cursor();
    let should_snapshot = window
        .buffer_view
        .with_buffer(|buffer| !buffer.current_text_matches_undo_head())
        .unwrap_or(false);

    if should_snapshot {
        window
            .buffer_view
            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
            .unwrap_or(());
    }
}

fn buffer_text(view: &BufferView) -> String {
    view.with_buffer(|buffer| buffer.as_str())
        .unwrap_or_default()
}

fn temp_path_with_ext(name: &str, ext: &str) -> AbsolutePath {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "urvim-window-tests-{}-{}-{}.{}",
        std::process::id(),
        nanos,
        name,
        ext
    ));
    AbsolutePath::from_path(path.as_path()).unwrap()
}

fn themed_window() -> Theme {
    let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
    let mut highlights = HighlightStyles::default();
    highlights.insert(
        Tag::parse("ui.status_bar").expect("valid tag"),
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
    );
    highlights.insert(
        Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
    );
    highlights.insert(
        Tag::parse("ui.selection").expect("valid tag"),
        Style::new().reverse(),
    );
    highlights.insert(
        Tag::parse("ui.window.active_line").expect("valid tag"),
        Style::new().bg(Color::ansi(21)),
    );
    highlights.insert(
        Tag::parse("ui.tab.active").expect("valid tag"),
        Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
    );
    highlights.insert(
        Tag::parse("ui.tab.inactive").expect("valid tag"),
        Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
    );
    highlights.insert(
        Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
        Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
    );
    highlights.insert(
        Tag::parse("ui.window.gutter").expect("valid tag"),
        Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
    );
    highlights.insert(
        Tag::parse("ui.window").expect("valid tag"),
        Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
    );
    highlights.insert(
        Tag::parse("ui.window.lines").expect("valid tag"),
        Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
    );
    highlights.insert(
        Tag::parse("ui.window.lines.resize").expect("valid tag"),
        Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
    );
    for tag_name in [
        "syntax.comment",
        "syntax.constant",
        "syntax.function",
        "syntax.keyword",
        "syntax.operator",
        "syntax.punctuation",
        "syntax.string",
        "syntax.type",
        "syntax.variable",
    ] {
        highlights.insert(Tag::parse(tag_name).expect("valid tag"), Style::new());
    }

    Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
}

fn syntax_themed_window() -> Theme {
    let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
    let mut highlights = HighlightStyles::default();
    highlights.insert(
        Tag::parse("ui.status_bar").expect("valid tag"),
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
    );
    highlights.insert(
        Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
    );
    highlights.insert(
        Tag::parse("ui.selection").expect("valid tag"),
        Style::new().reverse(),
    );
    highlights.insert(
        Tag::parse("ui.window.active_line").expect("valid tag"),
        Style::new().bg(Color::ansi(21)),
    );
    highlights.insert(
        Tag::parse("ui.tab.active").expect("valid tag"),
        Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
    );
    highlights.insert(
        Tag::parse("ui.tab.inactive").expect("valid tag"),
        Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
    );
    highlights.insert(
        Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
        Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
    );
    highlights.insert(
        Tag::parse("ui.window.gutter").expect("valid tag"),
        Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
    );
    highlights.insert(
        Tag::parse("ui.window").expect("valid tag"),
        Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
    );
    highlights.insert(
        Tag::parse("ui.window.lines").expect("valid tag"),
        Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
    );
    highlights.insert(
        Tag::parse("ui.window.lines.resize").expect("valid tag"),
        Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
    );
    for (tag_name, color) in [
        ("syntax.comment", 20),
        ("syntax.constant", 21),
        ("syntax.function", 22),
        ("syntax.keyword", 23),
        ("syntax.operator", 25),
        ("syntax.punctuation", 26),
        ("syntax.string", 27),
        ("syntax.string.escape", 30),
        ("syntax.type", 28),
        ("syntax.variable", 29),
    ] {
        highlights.insert(
            Tag::parse(tag_name).expect("valid tag"),
            Style::new().fg(Color::ansi(color)),
        );
    }
    highlights.insert(
        Tag::parse("syntax.markup").expect("valid tag"),
        Style::new().fg(Color::ansi(24)),
    );

    Theme::new("demo-syntax", ThemeKind::Ansi256, default_style, highlights)
}

fn syntax_themed_window_with_string_background() -> Theme {
    let mut theme = syntax_themed_window();
    theme.highlights.insert(
        Tag::parse("syntax.string").expect("valid tag"),
        Style::new().fg(Color::ansi(27)).bg(Color::ansi(40)),
    );
    theme
}

fn syntax_worker_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn repeated_rust_buffer(lines: usize) -> String {
    std::iter::repeat_n(
        "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
        lines,
    )
    .collect::<Vec<_>>()
    .join("\n")
}

fn todo_marker_themed_window() -> Theme {
    let mut theme = syntax_themed_window();
    theme
        .highlights
        .insert(tag("comment.todo"), Style::new().fg(Color::ansi(31)));
    theme
        .highlights
        .insert(tag("comment.fixme"), Style::new().fg(Color::ansi(32)));
    theme
}

fn tag(value: &str) -> Tag {
    Tag::parse(value).expect("valid tag")
}

fn pairing_test_config(auto_close_pairs: bool) -> Config {
    Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    }
}

fn auto_indent_test_config(auto_indent: AutoIndentMode) -> Config {
    Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    }
}

fn visual_test_setup() -> (impl Drop, impl Drop) {
    let theme = themed_window();
    let theme_guard = globals::set_test_active_theme(theme);
    let config_guard = globals::set_test_config(Config {
        theme: "demo".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });
    (theme_guard, config_guard)
}

fn apply_undo(window: &mut Window) {
    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
}

fn apply_undo_synced(window: &mut Window) {
    let cursor = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
        .expect("undo should restore previous state");
    window.set_cursor_synced(cursor);
}

fn apply_redo(window: &mut Window) {
    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.redo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
}

fn rendered_line<'a>(window: &'a Window, idx: usize) -> &'a [RenderChunk] {
    window
        .render_data()
        .get_line(idx)
        .expect("rendered line should exist")
}

#[test]
fn test_position_default() {
    let pos = Position::default();
    assert_eq!(pos.row, 0);
    assert_eq!(pos.col, 0);
}

#[test]
fn test_position_new() {
    let pos = Position::new(5, 10);
    assert_eq!(pos.row, 5);
    assert_eq!(pos.col, 10);
}

#[test]
fn test_size_default() {
    let size = Size::default();
    assert_eq!(size.rows, 0);
    assert_eq!(size.cols, 0);
}

#[test]
fn test_size_new() {
    let size = Size::new(24, 80);
    assert_eq!(size.rows, 24);
    assert_eq!(size.cols, 80);
}

#[test]
fn test_buffer_view_new() {
    let buffer = Buffer::from_str("test");
    let view = BufferView::new(buffer);

    assert_eq!(view.scroll_offset(), Position::default());
    assert_eq!(view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_buffer_view_cursor() {
    let buffer = Buffer::from_str("test");
    let mut view = BufferView::new(buffer);

    view.set_cursor(Cursor::new(0, 2));
    assert_eq!(view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_buffer_view_scroll_offset() {
    let buffer = Buffer::from_str("test");
    let mut view = BufferView::new(buffer);

    view.set_scroll_offset(Position::new(5, 10));
    assert_eq!(view.scroll_offset(), Position::new(5, 10));
}

#[test]
fn test_scroll_margin_starts_vertical_scrolling_near_bottom_and_top_edges() {
    let buffer = Buffer::from_str(
        &(0..80)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut view = BufferView::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        scroll_margin: ScrollMargin {
            vertical: 5,
            horizontal: 5,
        },
        ..Default::default()
    });

    view.set_cursor(Cursor::new(14, 0));
    view.scroll_to_cursor(Size::new(20, 40), 0);
    assert_eq!(view.scroll_offset().row, 0);

    view.set_cursor(Cursor::new(15, 0));
    view.scroll_to_cursor(Size::new(20, 40), 0);
    assert_eq!(view.scroll_offset().row, 1);

    view.set_scroll_offset(Position::new(10, 0));
    view.set_cursor(Cursor::new(15, 0));
    view.scroll_to_cursor(Size::new(20, 40), 0);
    assert_eq!(view.scroll_offset().row, 10);

    view.set_cursor(Cursor::new(14, 0));
    view.scroll_to_cursor(Size::new(20, 40), 0);
    assert_eq!(view.scroll_offset().row, 9);
}

#[test]
fn test_scroll_margin_starts_horizontal_scrolling_near_right_and_left_edges() {
    let buffer = Buffer::from_str(
        &(0..8)
            .map(|_| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789")
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut view = BufferView::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        scroll_margin: ScrollMargin {
            vertical: 5,
            horizontal: 5,
        },
        ..Default::default()
    });

    view.set_cursor(Cursor::new(0, 14));
    view.scroll_to_cursor(Size::new(6, 20), 0);
    assert_eq!(view.scroll_offset().col, 0);

    view.set_cursor(Cursor::new(0, 15));
    view.scroll_to_cursor(Size::new(6, 20), 0);
    assert_eq!(view.scroll_offset().col, 1);

    view.set_scroll_offset(Position::new(0, 20));
    view.set_cursor(Cursor::new(0, 25));
    view.scroll_to_cursor(Size::new(6, 20), 0);
    assert_eq!(view.scroll_offset().col, 20);

    view.set_cursor(Cursor::new(0, 24));
    view.scroll_to_cursor(Size::new(6, 20), 0);
    assert_eq!(view.scroll_offset().col, 19);
}

#[test]
fn test_scroll_margin_clamps_for_small_viewports() {
    let buffer = Buffer::from_str(
        &(0..20)
            .map(|_| "abcdefghijklmnopqrstuvwxyz")
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut view = BufferView::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        scroll_margin: ScrollMargin {
            vertical: 10,
            horizontal: 10,
        },
        ..Default::default()
    });

    view.set_cursor(Cursor::new(2, 2));
    view.scroll_to_cursor(Size::new(5, 4), 0);
    assert_eq!(view.scroll_offset(), Position::new(0, 0));

    view.set_cursor(Cursor::new(3, 3));
    view.scroll_to_cursor(Size::new(5, 4), 0);
    assert_eq!(view.scroll_offset(), Position::new(1, 1));
}

#[test]
fn test_scroll_margin_zero_keeps_edge_trigger_behavior() {
    let buffer = Buffer::from_str(
        &(0..20)
            .map(|_| "abcdefghijklmnopqrstuvwxyz")
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut view = BufferView::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        scroll_margin: ScrollMargin {
            vertical: 0,
            horizontal: 0,
        },
        ..Default::default()
    });

    view.set_cursor(Cursor::new(5, 9));
    view.scroll_to_cursor(Size::new(6, 10), 0);
    assert_eq!(view.scroll_offset(), Position::new(0, 0));

    view.set_cursor(Cursor::new(6, 10));
    view.scroll_to_cursor(Size::new(6, 10), 0);
    assert_eq!(view.scroll_offset(), Position::new(1, 1));
}

#[test]
fn test_buffer_view_syntax_label() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let view = BufferView::new(buffer);

    assert_eq!(view.syntax_label(), "Rust");
}

#[test]
fn test_buffer_view_syntax_label_uses_plain_text_for_missing_buffer() {
    let view = BufferView::from_buffer_id(BufferId::new(usize::MAX));

    assert_eq!(view.syntax_label(), "Plain Text");
}

#[test]
fn test_buffer_view_modified_state_tracks_buffer() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.txt")).unwrap();
    let mut buffer = Buffer::from_str_with_path("hello", path);
    assert!(!buffer.is_modified());
    buffer.insert_char(Cursor::new(0, 5), '!');
    let view = BufferView::new(buffer);

    assert!(view.is_modified());
}

#[test]
fn test_window_new() {
    let buffer = Buffer::from_str("test");
    let window = Window::new(buffer);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_window_render() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let mut window = Window::new(buffer);

    let mut screen = crate::screen::Screen::new(3, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 80));

    // With gutter (5 columns: digits(3) + 2 + fold sign), buffer starts at col 5
    // Check gutter background is rendered
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    // Check buffer content starts after gutter
    assert_eq!(screen.get_cell_mut(0, CONTENT_COL).unwrap().text, "l");
    assert_eq!(screen.get_cell_mut(1, CONTENT_COL).unwrap().text, "l");
}

#[test]
fn test_window_render_uses_theme_styles() {
    let buffer = Buffer::from_str("line1");
    let mut window = Window::new(buffer);
    let theme = themed_window();
    let expected_gutter_style = theme.resolve_name_with_default("ui.window.gutter");
    let expected_default_style = theme.default_style();
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(1, 12);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 12));

    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        expected_gutter_style
    );
    assert_eq!(
        screen.get_cell_mut(0, CONTENT_COL + 1).unwrap().style,
        expected_default_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 8).unwrap().style,
        expected_default_style
    );
}

#[test]
fn test_window_render_highlights_active_line_in_normal_mode() {
    let path = temp_path_with_ext("active-line", "rs");
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_line_fill_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_name("ui.window.active_line"));
    let expected_keyword_style = Style::new().fg(Color::ansi(23)).bg(Color::ansi(21));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, CONTENT_COL + 1).unwrap().style,
        expected_keyword_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 18).unwrap().style,
        expected_line_fill_style
    );
}

#[test]
fn test_window_render_keeps_explicit_token_background_on_active_line() {
    let path = temp_path_with_ext("active-line-background", "rs");
    let buffer = Buffer::from_str_with_path("let s = \"hi\";", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window_with_string_background();
    let expected_keyword_style = Style::new().fg(Color::ansi(23)).bg(Color::ansi(21));
    let expected_string_style = Style::new().fg(Color::ansi(27)).bg(Color::ansi(40));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, CONTENT_COL + 1).unwrap().style,
        expected_keyword_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 14).unwrap().style,
        expected_string_style
    );
}

#[test]
fn test_window_render_keeps_todo_marker_above_active_line_base_style() {
    let path = temp_path_with_ext("todo-active-line", "rs");
    let buffer = Buffer::from_str_with_path("fn main() { // TODO FIXME }", path);
    let mut window = Window::new(buffer);
    let theme = todo_marker_themed_window();
    let expected_todo_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_name("ui.window.active_line"))
        .overlay(theme.highlight_style_for_tag(&tag("comment.todo")));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        active_line: true,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    assert_eq!(
        screen.get_cell_mut(0, 21).unwrap().style,
        expected_todo_style
    );
}

#[test]
fn test_window_render_keeps_inlay_hint_background_on_active_line() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_inlay_hint(Cursor::new(0, 2), crate::buffer::Gravity::Right, "hint");
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_hint_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_name("ui.window.active_line"))
        .overlay(theme.highlight_style_for_name("ui.inlay_hint"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: false,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, 5).unwrap().style,
        expected_hint_style
    );
}

#[test]
fn test_window_render_ghost_text_inherits_active_line_background() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "ghost");
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_ghost_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_name("ui.window.active_line"))
        .overlay(
            Style::new()
                .set_foreground(theme.default_style().foreground())
                .faint()
                .italic(),
        );
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: false,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, CONTENT_COL + 2).unwrap().style,
        expected_ghost_style
    );
}

#[test]
fn test_window_render_records_buffer_visual_generation() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_inlay_hint(Cursor::new(0, 2), crate::buffer::Gravity::Right, "hint");
    let expected_generation = buffer.visual_generation();
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        syntax: false,
        ..Default::default()
    });

    assert_eq!(window.buffer_view().rendered_visual_generation(), 0);

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        window.buffer_view().rendered_visual_generation(),
        expected_generation
    );
}

#[test]
fn test_window_render_keeps_active_gutter_style_in_insert_mode() {
    let path = temp_path_with_ext("active-line-insert", "rs");
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_tag(&tag("keyword")));
    let expected_gutter_style = theme
        .resolve_name_with_default("ui.window.gutter")
        .overlay(theme.highlight_style_for_name("ui.window.gutter.active_line"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: true,
        ..Default::default()
    });
    window.switch_mode(ModeKind::Insert);

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        expected_gutter_style
    );
    assert_eq!(screen.get_cell_mut(0, 7).unwrap().style, expected_style);
}

#[test]
fn test_window_render_skips_active_line_when_disabled() {
    let path = temp_path_with_ext("active-line-disabled", "rs");
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_tag(&tag("keyword")));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: false,
        syntax: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    assert_eq!(
        screen.get_cell_mut(0, CONTENT_COL + 1).unwrap().style,
        expected_style
    );
}

#[test]
fn test_window_render_refreshes_visible_syntax_after_edit() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("visible-syntax-refresh", "rs").as_path())
            .unwrap();
    let buffer = Buffer::from_str_with_path("let value = true;\nlet other = false;", path);
    let mut window = Window::new(buffer);
    let buffer_id = window.buffer_view().buffer_id();
    let mut second_window = Window::from_buffer_id(buffer_id);

    let theme = syntax_themed_window();
    let expected_default_style = theme.default_style();
    let expected_comment_style =
        expected_default_style.overlay(theme.highlight_style_for_tag(&tag("comment")));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        syntax: true,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut prime_screen = crate::screen::Screen::new(1, 24);
    window.render(&mut prime_screen, Position::new(0, 0), Size::new(1, 24));

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(0, 0), "// "))
        .unwrap();

    let mut first_screen = crate::screen::Screen::new(1, 24);
    window.render(&mut first_screen, Position::new(0, 0), Size::new(1, 24));
    let mut second_screen = crate::screen::Screen::new(1, 24);
    second_window.render(&mut second_screen, Position::new(0, 0), Size::new(1, 24));

    let first_cell = first_screen.get_cell_mut(0, CONTENT_COL + 1).unwrap();
    let second_cell = second_screen.get_cell_mut(0, CONTENT_COL + 1).unwrap();
    let offscreen_cache = window
        .buffer_view()
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(1))
        .unwrap();

    assert_eq!(first_cell.text, "/");
    assert_eq!(second_cell.text, "/");
    assert_eq!(first_cell.style, expected_comment_style);
    assert_eq!(second_cell.style, expected_comment_style);
    assert!(offscreen_cache.is_some());
    assert!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.syntax_cache_complete())
            .unwrap_or(false)
    );
}

#[test]
fn test_window_render_refreshes_scrolled_visible_syntax_after_edit() {
    let _lock = syntax_worker_lock();
    let path = AbsolutePath::from_path(
        temp_path_with_ext("scrolled-visible-syntax-refresh", "rs").as_path(),
    )
    .unwrap();
    let buffer = Buffer::from_str_with_path(
        "let first = true;\nlet second = false;\nlet third = true;\nlet fourth = false;\nlet fifth = true;",
        path,
    );
    let mut window = Window::new(buffer);
    let buffer_id = window.buffer_view().buffer_id();
    let mut second_window = Window::from_buffer_id(buffer_id);
    let theme = syntax_themed_window();
    let expected_comment_style = theme.highlight_style_for_tag(&tag("comment"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        syntax: true,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut prime_screen = crate::screen::Screen::new(2, 24);
    window.set_cursor(Cursor::new(3, 0));
    window.render(&mut prime_screen, Position::new(0, 0), Size::new(2, 24));

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(2, 0), "// "))
        .unwrap();

    let mut screen = crate::screen::Screen::new(2, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(2, 24));
    let mut second_screen = crate::screen::Screen::new(2, 24);
    second_window.render(&mut second_screen, Position::new(0, 0), Size::new(2, 24));

    let rendered_line = rendered_line(&window, 0);
    assert!(
        rendered_line
            .iter()
            .any(|chunk| chunk.text.starts_with("//") && chunk.style == expected_comment_style)
    );
    let cached_line = window
        .buffer_view()
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(2))
        .unwrap();
    assert!(cached_line.is_some());
}

#[test]
fn test_window_render_expands_tabs_using_configured_width() {
    let buffer = Buffer::from_str("a\tb");
    let window = Window::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        tab_width: 4,
        ..Default::default()
    });

    let render_data = window
        .buffer_view()
        .build_render_data_with_style(Size::new(1, 8), Style::default());
    let mut screen = crate::screen::Screen::new(1, 8);
    render_data.render(
        &mut screen,
        Position::new(0, 0),
        Size::new(1, 8),
        Style::default(),
    );

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 5).unwrap().text, "b");
}

#[test]
fn test_window_render_expands_leading_tabs_after_gutter() {
    let buffer = Buffer::from_str("\tX\n\n\n\n\n\n\n\n\n");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(Config {
        tab_width: 4,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(2, 12);
    window.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(screen.get_cell_mut(0, 5).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 6).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 7).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 8).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, "X");
}

#[test]
fn test_window_render_collapses_indent_scope_fold() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter\ntail"));
    window.set_cursor(Cursor::new(1, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(4, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 40));

    let rendered = window
        .render_data()
        .get_line(0)
        .expect("folded start line should render")
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<String>();

    assert!(rendered.contains("outer"));
    assert!(rendered.contains("... 2 lines folded"));
    assert_eq!(window.render_data().line_data[1].buffer_line, 3);
}

#[test]
fn test_window_render_fold_text_inherits_active_line_background() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter"));
    window.set_cursor(Cursor::new(0, 0));
    let theme = syntax_themed_window();
    let expected_fold_style = theme
        .default_style()
        .overlay(theme.highlight_style_for_name("ui.window.active_line"))
        .overlay(
            Style::new()
                .set_foreground(theme.default_style().foreground())
                .faint()
                .italic(),
        );
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        syntax: false,
        ..Default::default()
    });

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(4, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 40));

    assert_eq!(
        screen
            .get_cell_mut(0, CONTENT_COL + "outer".len() as u16)
            .unwrap()
            .style,
        expected_fold_style
    );
}

#[test]
fn test_window_render_fold_spans_blank_lines() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n\n  inner2\nafter"));
    window.set_cursor(Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(5, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 40));

    let rendered = window
        .render_data()
        .get_line(0)
        .expect("folded start line should render")
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<String>();

    assert!(rendered.contains("... 3 lines folded"));
    assert_eq!(window.render_data().line_data[1].buffer_line, 4);
}

#[test]
fn test_window_render_does_not_fold_blank_only_suffix() {
    let mut window = Window::new(Buffer::from_str("outer\n\n"));
    window.set_cursor(Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(3, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 20));
    assert!(
        window.render_data().line_data[0]
            .folded_line_count
            .is_none()
    );
}

#[test]
fn test_window_fold_state_is_window_local() {
    let buffer = Buffer::from_str("outer\n  inner1\n  inner2\nafter");
    let mut folded = Window::from_owned_buffer(buffer.clone());
    let unfolded = Window::from_owned_buffer(buffer);
    folded.set_cursor(Cursor::new(1, 0));

    assert_eq!(
        folded.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );

    let mut folded_screen = crate::screen::Screen::new(4, 40);
    folded.render(&mut folded_screen, Position::new(0, 0), Size::new(4, 40));
    let folded_data = folded.render_data();
    let unfolded_data = unfolded
        .buffer_view()
        .build_render_data_with_style(Size::new(4, 40), Style::default());

    assert_eq!(folded_data.line_data[1].buffer_line, 3);
    assert_eq!(unfolded_data.line_data[1].buffer_line, 1);
}

#[test]
fn test_window_move_down_skips_folded_lines() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter"));
    window.set_cursor(Cursor::new(1, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveDown)),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor().line, 3);
}

#[test]
fn test_window_move_up_skips_large_fold_without_scanning_each_line() {
    let mut text = String::from("outer\n");
    for idx in 0..1000 {
        text.push_str(&format!("  inner{idx}\n"));
    }
    text.push_str("after");
    let mut window = Window::new(Buffer::from_str(&text));
    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    window.set_cursor(Cursor::new(1001, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveUp)),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor().line, 0);
}

#[test]
fn test_counted_line_jump_opens_fold_containing_target() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter"));
    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );

    assert_eq!(
        window.dispatch_action(&Action::count(
            3,
            Box::new(Action::new(ActionKind::MoveToLastLine)),
        )),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(4, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 40));

    assert_eq!(window.buffer_view().cursor().line, 2);
    assert_eq!(window.render_data().line_data[1].buffer_line, 1);
    assert_eq!(window.render_data().line_data[2].buffer_line, 2);
}

#[test]
fn test_move_left_from_fold_closing_line_stays_on_line_start() {
    let mut window = Window::new(Buffer::from_str("struct Cli {\n    files: Vec<File>,\n}"));
    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    window.set_cursor(Cursor::new(2, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveLeft)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(3, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    assert_eq!(window.buffer_view().cursor(), Cursor::new(2, 0));
    assert_eq!(window.render_data().line_data[1].buffer_line, 2);
}

#[test]
fn test_open_line_below_opens_fold_when_cursor_enters_hidden_line() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter"));
    window.set_cursor(Cursor::new(1, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::OpenLineBelow)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(5, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 40));

    assert_eq!(window.buffer_view().cursor().line, 1);
    assert_eq!(window.render_data().line_data[1].buffer_line, 1);
    assert_eq!(window.render_data().line_data[2].buffer_line, 2);
}

#[test]
fn test_open_line_above_opens_fold_when_cursor_enters_hidden_line() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner1\n  inner2\nafter"));
    window.set_cursor(Cursor::new(1, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    window.set_cursor(Cursor::new(3, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::OpenLineAbove)),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(5, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 40));

    assert_eq!(window.buffer_view().cursor().line, 3);
    assert_eq!(window.render_data().line_data[2].buffer_line, 2);
    assert_eq!(window.render_data().line_data[3].buffer_line, 3);
}

#[test]
fn test_open_line_above_on_brace_fold_end_inserts_above_brace() {
    let mut window = Window::new(Buffer::from_str(
        "struct Cli {\n    theme: Option<String>,\n    no_syntax: bool,\n    files: Vec<File>,\n}\n",
    ));
    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    window.set_cursor(Cursor::new(4, 0));

    let mut folded_screen = crate::screen::Screen::new(3, 40);
    window.render(&mut folded_screen, Position::new(0, 0), Size::new(3, 40));
    assert_eq!(window.render_data().line_data[1].buffer_line, 4);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::OpenLineAbove)),
        ActionResult::Handled
    );

    let text = window
        .buffer_view()
        .with_buffer(|buffer| buffer.as_str())
        .expect("buffer should exist");

    assert_eq!(window.buffer_view().cursor().line, 4);
    assert_eq!(
        text,
        "struct Cli {\n    theme: Option<String>,\n    no_syntax: bool,\n    files: Vec<File>,\n\n}"
    );
}

#[test]
fn test_counted_line_jump_opens_all_nested_folds() {
    let buffer = Buffer::from_str("outer\n  inner\n    deep\nafter");
    let mut window = Window::from_owned_buffer(buffer);
    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );
    window.set_cursor(Cursor::new(1, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::CloseFold)),
        ActionResult::Handled
    );

    assert_eq!(
        window.dispatch_action(&Action::count(
            3,
            Box::new(Action::new(ActionKind::MoveToLastLine)),
        )),
        ActionResult::Handled
    );
    let mut screen = crate::screen::Screen::new(4, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 40));

    assert_eq!(window.buffer_view().cursor().line, 2);
    assert_eq!(window.render_data().line_data[1].buffer_line, 1);
    assert_eq!(window.render_data().line_data[2].buffer_line, 2);
}

#[test]
fn test_window_render_draws_indent_guide_between_scope_boundaries() {
    let buffer = Buffer::from_str("  outer\n    inner1\n\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 2));
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    assert_ne!(screen.get_cell_mut(0, 7).unwrap().text, "|");
    assert_eq!(screen.get_cell_mut(1, 7).unwrap().text, "|");
    assert_eq!(screen.get_cell_mut(2, 7).unwrap().text, "|");
    assert_eq!(screen.get_cell_mut(3, 7).unwrap().text, "|");
    assert_ne!(screen.get_cell_mut(4, 7).unwrap().text, "|");
}

#[test]
fn test_window_render_skips_indent_guide_when_disabled() {
    let buffer = Buffer::from_str("  outer\n    inner1\n\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 2));
    let _config_guard = globals::set_test_config(Config {
        indent_guides: false,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    assert_ne!(screen.get_cell_mut(1, 7).unwrap().text, "|");
    assert_ne!(screen.get_cell_mut(2, 7).unwrap().text, "|");
    assert_ne!(screen.get_cell_mut(3, 7).unwrap().text, "|");
}

#[test]
fn test_window_render_uses_unicode_indent_glyph_when_capability_enabled() {
    let buffer = Buffer::from_str("  outer\n    inner1\n\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 2));
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::UnicodeIndent]),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    assert_eq!(screen.get_cell_mut(2, 7).unwrap().text, "│");
}

#[test]
fn test_window_render_indent_guide_uses_split_border_style() {
    let buffer = Buffer::from_str("  outer\n    inner1\n\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 2));
    let theme = themed_window();
    let expected_style = theme
        .default_style()
        .accent(theme.resolve_name_with_default("ui.window.lines.indent"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    let cell = screen.get_cell_mut(2, 7).unwrap();
    assert_eq!(cell.text, "|");
    assert_eq!(cell.style, expected_style);
}

#[test]
fn test_window_render_indent_guide_uses_visual_column_for_tabs() {
    let buffer = Buffer::from_str(" \touter\n \t\tinner\n\n \t\tclose\n \tend\nend");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 2));
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        tab_width: 4,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    assert_eq!(screen.get_cell_mut(2, 10).unwrap().text, "|");
}

#[test]
fn test_window_render_indent_guide_preserves_visual_selection_background() {
    let mut theme = themed_window();
    theme.highlights.insert(
        Tag::parse("ui.selection").expect("valid tag"),
        Style::new().bg(Color::ansi(99)),
    );
    let selection_style = theme.highlight_style_for_name("ui.selection");
    let guide_style = theme.resolve_name_with_default("ui.window.lines.indent");
    let expected_style = selection_style.accent(guide_style);
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        ..Default::default()
    });

    let buffer = Buffer::from_str("  outer\n    inner1\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 3));

    let mut screen = crate::screen::Screen::new(5, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 24));

    let cell = screen.get_cell_mut(1, 7).unwrap();
    assert_eq!(cell.text, "|");
    assert_eq!(cell.style, expected_style);
}

#[test]
fn test_window_render_skips_indent_guide_without_eligible_scope() {
    let buffer = Buffer::from_str("  outer\n    inner1\n\n    inner2\n  close\nafter");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 0));
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(6, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 24));

    assert_ne!(screen.get_cell_mut(1, 7).unwrap().text, "|");
    assert_ne!(screen.get_cell_mut(2, 7).unwrap().text, "|");
    assert_ne!(screen.get_cell_mut(3, 7).unwrap().text, "|");
}

#[test]
fn test_window_render_indent_guide_does_not_overwrite_wrapped_continuation_text() {
    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        wrap_mode: WrapMode::Hard,
        ..Default::default()
    });
    let mut window = Window::new(Buffer::from_str("a\n  abcdefghij\n  tail\nz"));
    window.set_wrap_enabled(true);
    window.set_cursor(Cursor::new(1, 2));

    let mut screen = crate::screen::Screen::new(6, 9);
    window.render(&mut screen, Position::new(0, 0), Size::new(6, 9));

    // Row 2 is the second wrapped segment of line 2 ("efghij"), and the guide
    // column intersects that segment. The rendered text must win over the guide.
    assert_eq!(screen.get_cell_mut(2, 8).unwrap().text, "f");
}

#[test]
fn test_window_render_fills_empty_content_rows_with_theme_default() {
    let buffer = Buffer::from_str("line1");
    let mut window = Window::new(buffer);
    let theme = themed_window();
    let expected_gutter_style = theme.resolve_name_with_default("ui.window.gutter");
    let expected_default_style = theme.default_style();
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(3, 12);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 12));

    assert_eq!(
        screen.get_cell_mut(1, 0).unwrap().style,
        expected_gutter_style
    );
    assert_eq!(
        screen.get_cell_mut(1, CONTENT_COL).unwrap().style,
        expected_default_style
    );
    assert_eq!(
        screen.get_cell_mut(2, CONTENT_COL).unwrap().style,
        expected_default_style
    );
}

#[test]
fn test_window_render_uses_syntax_styles_for_supported_filetypes() {
    let path = temp_path_with_ext("syntax", "rs");
    let buffer = Buffer::from_str_with_path(
        "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
        path,
    );
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_keyword_style = theme.highlight_style_for_tag(&tag("keyword"));
    let expected_constant_style = theme.highlight_style_for_tag(&tag("constant"));
    let expected_type_style = theme.highlight_style_for_tag(&tag("type"));
    let expected_variable_style = theme.highlight_style_for_tag(&tag("variable"));
    let expected_string_style = theme.highlight_style_for_tag(&tag("string"));
    let expected_comment_style = theme.highlight_style_for_tag(&tag("comment"));
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "fn" && chunk.style == expected_keyword_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "Some" && chunk.style == expected_constant_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "Option" && chunk.style == expected_type_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "value" && chunk.style == expected_variable_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("hi") && chunk.style == expected_string_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("note") && chunk.style == expected_comment_style)
    );
}

#[test]
fn test_window_render_omits_syntax_styles_when_disabled() {
    let path = temp_path_with_ext("syntax-disabled", "rs");
    let buffer = Buffer::from_str_with_path(
        "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
        path,
    );
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_default_style = Style::default();
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: false,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);

    assert!(!line.is_empty());
    assert!(
        line.iter()
            .all(|chunk| chunk.style == expected_default_style)
    );
    assert!(line.iter().any(|chunk| chunk.text.contains("fn main()")));
    assert!(
        !window
            .buffer_view()
            .with_buffer(|buffer| buffer.syntax_background_pending())
            .unwrap_or(true)
    );
}

#[test]
fn test_window_render_does_not_force_full_syntax_warmup_on_bottom_jump() {
    let _lock = syntax_worker_lock();
    let theme = syntax_themed_window();
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let path = temp_path_with_ext("bottom-jump", "rs");
    let buffer = Buffer::from_str_with_path(&repeated_rust_buffer(256), path);
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(255, 0));

    let mut screen = crate::screen::Screen::new(4, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 80));

    let rendered_line = rendered_line(&window, 0);
    let syntax_pending = window
        .buffer_view()
        .with_buffer(|buffer| buffer.syntax_background_pending())
        .unwrap_or(false);
    let cache_complete = window
        .buffer_view()
        .with_buffer(|buffer| buffer.syntax_cache_complete())
        .unwrap_or(true);
    let beyond_viewport_cached = window
        .buffer_view()
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(200).is_some())
        .unwrap_or(true);
    let eof_line_cached = window
        .buffer_view()
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(255).is_some())
        .unwrap_or(true);

    assert!(!rendered_line.is_empty());
    assert!(
        rendered_line
            .iter()
            .any(|chunk| chunk.text.contains("fn main()"))
    );
    assert!(syntax_pending);
    assert!(!cache_complete);
    assert!(!beyond_viewport_cached);
    assert!(!eof_line_cached);
}

#[test]
fn test_window_render_keeps_bottom_viewport_highlighted_after_completion_top_edit() {
    let _lock = syntax_worker_lock();
    let theme = syntax_themed_window();
    let expected_keyword_style = theme.highlight_style_for_tag(&tag("keyword"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new();\n}}\n{body}");
    let path = temp_path_with_ext("completion-render-fallback", "rs");
    let buffer = Buffer::from_str_with_path(&source, path);
    let mut window = Window::new(buffer);

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1))
        })
        .unwrap();

    let completion_line = window
        .buffer_view()
        .with_buffer(|buffer| {
            (0..buffer.line_count())
                .find(|line| {
                    buffer
                        .line_at(*line)
                        .is_some_and(|line_text| line_text.to_string().contains("let guard"))
                })
                .expect("guard line should exist")
        })
        .unwrap();
    let line_text = window
        .buffer_view()
        .with_buffer(|buffer| buffer.line_at(completion_line).map(|line| line.to_string()))
        .flatten()
        .expect("guard line should exist");
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let range = crate::buffer::TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_start + "guard".len()),
    };
    let additional_edits = vec![crate::ui::completion::CompletionTextEdit {
        range: crate::buffer::TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 0),
        },
        text: "use std::borrow::Cow;\n".to_string(),
    }];

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer
                .apply_completion(
                    range,
                    "guard_handle",
                    "guard_handle".len(),
                    &additional_edits,
                )
                .expect("completion should apply");
        })
        .unwrap();

    let filler_line_idx = window
        .buffer_view()
        .with_buffer(|buffer| {
            (0..buffer.line_count())
                .find(|line| {
                    buffer
                        .line_at(*line)
                        .is_some_and(|line_text| line_text.to_string().contains("fn filler_1023()"))
                })
                .expect("filler line should exist")
        })
        .unwrap();

    window.set_cursor(Cursor::new(filler_line_idx, 0));
    let mut screen = crate::screen::Screen::new(1, 120);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 120));

    let rendered_line = rendered_line(&window, 0);
    assert!(
        rendered_line
            .iter()
            .any(|chunk| chunk.text == "fn" && chunk.style == expected_keyword_style)
    );
}

#[test]
fn test_window_render_requests_async_syntax_without_warmup() {
    let _lock = syntax_worker_lock();
    let theme = syntax_themed_window();
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let path = temp_path_with_ext("top-warmup", "rs");
    let buffer = Buffer::from_str_with_path(&repeated_rust_buffer(256), path);
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(0, 0));

    let mut screen = crate::screen::Screen::new(4, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 80));

    let far_line_cached = window
        .buffer_view()
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(120).is_some())
        .unwrap_or(true);

    assert!(!far_line_cached);
}

#[test]
fn test_buffer_view_set_cursor_debug_logging_does_not_force_syntax_warmup() {
    let _lock = syntax_worker_lock();
    let _config_guard = globals::set_test_config(Config {
        syntax: true,
        ..Default::default()
    });
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_test_writer()
            .with_ansi(false)
            .without_time(),
    );
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);

    let path = temp_path_with_ext("set-cursor-debug", "rs");
    let buffer = Buffer::from_str_with_path(&repeated_rust_buffer(256), path);
    let mut view = BufferView::new(buffer);
    view.set_cursor(Cursor::new(255, 0));

    let eof_line_cached = view
        .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(255).is_some())
        .unwrap_or(true);
    let cache_line_count = view
        .with_buffer(|buffer| buffer.cached_syntax_line_count())
        .unwrap_or(usize::MAX);

    assert!(!eof_line_cached);
    assert_eq!(cache_line_count, 0);
}

#[test]
fn test_window_render_uses_background_syntax_after_tick() {
    let _lock = syntax_worker_lock();
    let theme = syntax_themed_window();
    let _theme_guard = globals::set_test_active_theme(theme.clone());
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });
    let gate = std::sync::Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    globals::with_buffer_pool(|pool| {
        pool.submit_background_job(
            JobKind::TestGate,
            JobToken::new(1),
            BackgroundJob::Gate {
                gate: std::sync::Arc::clone(&gate),
            },
        )
        .expect("gate job should submit");
    });

    let buffer = Buffer::from_str_with_path(
        &repeated_rust_buffer(64),
        temp_path_with_ext("background-syntax", "rs"),
    );
    let mut window = Window::new(buffer);
    let expected_keyword_style = theme.highlight_style_for_tag(&tag("keyword"));
    let expected_constant_style = theme.highlight_style_for_tag(&tag("constant"));
    let expected_type_style = theme.highlight_style_for_tag(&tag("type"));
    let expected_variable_style = theme.highlight_style_for_tag(&tag("variable"));
    let expected_string_style = theme.highlight_style_for_tag(&tag("string"));
    let expected_comment_style = theme.highlight_style_for_tag(&tag("comment"));
    let _expected_default_style = Style::default();

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);
    assert!(!line.is_empty());
    assert!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.syntax_background_pending())
            .unwrap_or(false)
    );

    {
        let (lock, cvar) = &*gate;
        let mut open = lock.lock().unwrap();
        *open = true;
        cvar.notify_all();
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut applied = false;
    while !applied {
        applied = globals::with_buffer_pool(|pool| pool.process_background_jobs());

        assert!(
            Instant::now() < deadline,
            "timed out waiting for syntax result"
        );
        thread::sleep(Duration::from_millis(5));
    }

    assert!(applied);

    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "fn" && chunk.style == expected_keyword_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "Some" && chunk.style == expected_constant_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "Option" && chunk.style == expected_type_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "value" && chunk.style == expected_variable_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("hi") && chunk.style == expected_string_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("note") && chunk.style == expected_comment_style)
    );
}

#[test]
fn test_window_render_distinguishes_rust_format_string_escapes() {
    let path = temp_path_with_ext("syntax-format-escape", "rs");
    let buffer = Buffer::from_str_with_path("let msg = format!(\"{{literal}}\");", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let expected_string_style = theme.highlight_style_for_tag(&tag("string"));
    let expected_escape_style = theme.highlight_style_for_tag(&tag("string.escape"));
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);

    assert!(
        line.iter()
            .any(|chunk| chunk.text == "literal" && chunk.style == expected_string_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "{{" && chunk.style == expected_escape_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "}}" && chunk.style == expected_escape_style)
    );
}

#[test]
fn test_window_render_highlights_todo_markers_inside_comments() {
    let path = temp_path_with_ext("todo-markers", "rs");
    let buffer = Buffer::from_str_with_path("fn main() { let value = 1; // TODO FIXME }", path);
    let mut window = Window::new(buffer);
    let theme = todo_marker_themed_window();
    let expected_keyword_style = theme.highlight_style_for_tag(&tag("keyword"));
    let expected_comment_style = theme.highlight_style_for_tag(&tag("comment"));
    let expected_todo_style = theme.highlight_style_for_tag(&tag("comment.todo"));
    let expected_fixme_style = theme.highlight_style_for_tag(&tag("comment.fixme"));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "fn" && chunk.style == expected_keyword_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "TODO" && chunk.style == expected_todo_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "FIXME" && chunk.style == expected_fixme_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("value") && chunk.style != expected_todo_style)
    );
    assert!(
        line.iter()
            .any(|chunk| chunk.text.contains("// ") && chunk.style == expected_comment_style)
    );
}

#[test]
fn test_window_render_skips_todo_markers_when_syntax_is_disabled() {
    let path = temp_path_with_ext("todo-markers-disabled", "rs");
    let buffer = Buffer::from_str_with_path("fn main() { // TODO FIXME }", path);
    let mut window = Window::new(buffer);
    let theme = todo_marker_themed_window();
    let expected_default_style = Style::default();
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: false,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(1, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 80));

    let line = rendered_line(&window, 0);
    assert!(!line.is_empty());
    assert!(
        line.iter()
            .all(|chunk| chunk.style == expected_default_style)
    );
    assert!(line.iter().any(|chunk| chunk.text.contains("TODO")));
}

#[test]
fn test_open_line_below_uses_neighbor_indent() {
    let mut window = Window::new(Buffer::from_str(
        "    fn main() {\n  println!(\"hi\");\n    }",
    ));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.handle_count(1, &Action::new(ActionKind::OpenLineBelow)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    fn main() {\n    \n  println!(\"hi\");\n    }"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 4));
}

#[test]
fn test_open_line_above_uses_neighbor_indent() {
    let mut window = Window::new(Buffer::from_str(
        "  fn main() {\n    println!(\"hi\");\n  }",
    ));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    window.set_cursor(Cursor::new(1, 4));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::OpenLineAbove)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "  fn main() {\n    \n    println!(\"hi\");\n  }"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 3));
}

#[test]
fn test_open_line_below_undo_restores_original_text() {
    let mut window = Window::new(Buffer::from_str("hello"));
    window.set_cursor(Cursor::new(0, 5));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::OpenLineBelow).with_to_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello\n");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("world".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello\nworld");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 5));

    commit_insert_exit_snapshot(&mut window);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_newline_uses_neighbor_indent_and_reports_suffix() {
    let mut window = Window::new(Buffer::from_str(
        "    fn main() {\n  println!(\"hi\");\n    }",
    ));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    let line_end = window.buffer_view().line_len(0);
    window.set_cursor(Cursor::new(0, line_end));

    assert_eq!(
        window.dispatch_action(&Action::insert_newline().with_from_mode(ModeKind::Insert)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    fn main() {\n    \n  println!(\"hi\");\n    }"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 4));
    assert_eq!(window.take_pending_repeat_suffix().as_deref(), Some("    "));
}

#[test]
fn test_change_line_preserves_current_indentation_when_auto_indent_is_enabled() {
    let mut window = Window::new(Buffer::from_str("    fn main() {\n  println!(\"hi\");"));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::ChangeLine).with_to_mode(ModeKind::Insert)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    \n  println!(\"hi\");"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
    assert_eq!(window.take_pending_repeat_suffix().as_deref(), Some("    "));
}

#[test]
fn test_change_line_undo_restores_original_text() {
    let mut window = Window::new(Buffer::from_str("    fn main() {\n  println!(\"hi\");"));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::ChangeLine).with_to_mode(ModeKind::Insert)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    \n  println!(\"hi\");"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
    assert_eq!(window.take_pending_repeat_suffix().as_deref(), Some("    "));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("x".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    x\n  println!(\"hi\");"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 5));

    commit_insert_exit_snapshot(&mut window);

    apply_undo(&mut window);

    assert_eq!(
        buffer_text(window.buffer_view()),
        "    fn main() {\n  println!(\"hi\");"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_change_to_line_end_undo_restores_original_text() {
    let mut window = Window::new(Buffer::from_str("hello world"));
    window.set_cursor(Cursor::new(0, 3));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::ChangeToLineEnd).with_to_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hel");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 3));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("p".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "help");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));

    commit_insert_exit_snapshot(&mut window);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "hello world");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_newline_reports_no_suffix_when_disabled() {
    let mut window = Window::new(Buffer::from_str(
        "    fn main() {\n  println!(\"hi\");\n    }",
    ));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Off));
    let line_end = window.buffer_view().line_len(0);
    window.set_cursor(Cursor::new(0, line_end));

    assert_eq!(
        window.dispatch_action(&Action::insert_newline().with_from_mode(ModeKind::Insert)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "    fn main() {\n\n  println!(\"hi\");\n    }"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
    assert_eq!(window.take_pending_repeat_suffix(), None);
}

#[test]
fn test_insert_newline_prefers_next_line_indent_when_it_is_more_indented() {
    let mut window = Window::new(Buffer::from_str("  if ready {\n    println!(\"hi\");"));
    let _config_guard = globals::set_test_config(auto_indent_test_config(AutoIndentMode::Neighbor));
    let line_end = window.buffer_view().line_len(0);
    window.set_cursor(Cursor::new(0, line_end));

    assert_eq!(
        window.dispatch_action(&Action::insert_newline().with_from_mode(ModeKind::Insert)),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "  if ready {\n    \n    println!(\"hi\");"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 4));
    assert_eq!(window.take_pending_repeat_suffix().as_deref(), Some("    "));
}

#[test]
fn test_raw_paste_in_insert_mode_bypasses_auto_pair_and_auto_indent() {
    let mut window = Window::new(Buffer::from_str("fn main() {"));
    let _config_guard = globals::set_test_config(Config {
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Neighbor,
        ..Default::default()
    });
    window.switch_mode(ModeKind::Insert);
    let cursor = Cursor::new(0, window.buffer_view().line_len(0));
    window.set_cursor(cursor);
    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
        .unwrap();

    process_action_and_snapshot(
        &mut window,
        &Action::insert_raw_paste("(\n".to_string()).with_from_mode(ModeKind::Insert),
    );

    assert_eq!(buffer_text(window.buffer_view()), "fn main() {(\n");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
    assert_eq!(window.take_pending_repeat_suffix(), None);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "fn main() {");
    assert_eq!(window.buffer_view().cursor(), cursor);
}

#[test]
fn test_raw_paste_in_normal_mode_inserts_text_without_mode_change() {
    let mut window = Window::new(Buffer::from_str("hello"));
    let cursor = Cursor::new(0, window.buffer_view().line_len(0));
    window.set_cursor(cursor);
    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
        .unwrap();

    process_action_and_snapshot(
        &mut window,
        &Action::insert_raw_paste(" world".to_string()).with_from_mode(ModeKind::Normal),
    );

    assert_eq!(buffer_text(window.buffer_view()), "hello world");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 10));
    assert_eq!(window.mode_kind(), ModeKind::Normal);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), cursor);
}

#[test]
fn test_raw_paste_replaces_visual_selection_and_exits_to_normal_mode() {
    let mut window = Window::new(Buffer::from_str("abcdef"));
    window.set_cursor(Cursor::new(0, 1));
    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.push_snapshot(Cursor::new(0, 1)))
        .unwrap();
    window.switch_mode(ModeKind::Visual);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 4));

    process_action_and_snapshot(
        &mut window,
        &Action::replace_selection_raw_paste("zap".to_string())
            .with_mode(Some(ModeKind::Visual), Some(ModeKind::Normal)),
    );
    window.switch_mode(ModeKind::Normal);

    assert_eq!(buffer_text(window.buffer_view()), "azapf");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
    assert_eq!(window.mode_kind(), ModeKind::Normal);
    assert_eq!(window.buffer_view().visual_selection(), None);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "abcdef");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 1));
}

#[test]
fn test_raw_paste_replaces_visual_line_selection_and_exits_to_normal_mode() {
    let mut window = Window::new(Buffer::from_str("abc\ndef\nghi"));
    window.set_cursor(Cursor::new(0, 0));
    window.switch_mode(ModeKind::VisualLine);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 0));

    process_action_and_snapshot(
        &mut window,
        &Action::replace_selection_raw_paste("Z".to_string())
            .with_mode(Some(ModeKind::VisualLine), Some(ModeKind::Normal)),
    );
    window.switch_mode(ModeKind::Normal);

    assert_eq!(buffer_text(window.buffer_view()), "Z\nghi");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
    assert_eq!(window.mode_kind(), ModeKind::Normal);
    assert_eq!(window.buffer_view().visual_selection(), None);
}

#[test]
fn test_indent_decrease_shifts_current_line() {
    let mut window = Window::new(Buffer::from_str("    hello\n  world"));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::IndentDecrease)),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello\n  world");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_counted_indent_decrease_shifts_multiple_lines() {
    let mut window = Window::new(Buffer::from_str("    hello\n        world\n  done"));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::IndentDecrease)
                .with_count(2)
                .expect("counted indent decrease should be allowed"),
        ),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "hello\n    world\n  done"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_mode_shift_tab_dedents_without_leaving_insert_mode() {
    let mut window = Window::new(Buffer::from_str("    hello"));
    window.set_cursor(Cursor::new(0, 4));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::IndentDecrease).with_from_mode(ModeKind::Insert),
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_mode_backspace_dedents_inside_leading_whitespace() {
    let mut window = Window::new(Buffer::from_str("    hello"));
    window.set_cursor(Cursor::new(0, 2));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::DeleteBackward).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_mode_backspace_keeps_plain_deletion_outside_indentation() {
    let mut window = Window::new(Buffer::from_str("    hello"));
    window.set_cursor(Cursor::new(0, 5));

    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::DeleteBackward).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "    ello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
}

#[test]
fn test_render_chunk_new() {
    let chunk = RenderChunk::new("test", crate::terminal::Style::default());
    assert_eq!(chunk.text, "test");
}

#[test]
fn test_render_chunk_default_text() {
    let chunk = RenderChunk::default_text("test");
    assert_eq!(chunk.text, "test");
    assert_eq!(chunk.style, crate::terminal::Style::default());
}

#[test]
fn test_render_data_new() {
    let data = RenderData::new(10);
    assert_eq!(data.line_count(), 0);
    assert_eq!(data.visible_rows(), 10);
}

#[test]
fn test_render_data_get_line() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 80));

    let line = render_data.get_line(0).unwrap();
    assert!(!line.is_empty());
    assert_eq!(line[0].text, "line1");
}

#[test]
fn test_render_data_inserts_ghost_text_inline() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "ghost");

    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(1, 80));
    let line = render_data.get_line(0).expect("line should render");

    assert_eq!(
        line.iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ab", "ghost", "cd"]
    );
    assert!(!line[0].is_ghost_text);
    assert!(line[1].is_ghost_text);
    assert!(!line[2].is_ghost_text);
}

#[test]
fn test_marker_mutations_advance_buffer_visual_generation() {
    let mut buffer = Buffer::from_str("abcd");
    let initial = buffer.visual_generation();

    let marker_id = buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "x");
    assert_ne!(buffer.visual_generation(), initial);

    let after_insert = buffer.visual_generation();
    buffer.remove_marker(marker_id);
    assert_ne!(buffer.visual_generation(), after_insert);
}

#[test]
fn test_render_data_adds_trailing_space_after_colon_inlay_hint() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_inlay_hint(Cursor::new(0, 2), crate::buffer::Gravity::Right, "name: ");

    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(1, 80));
    let line = render_data.get_line(0).expect("line should render");

    assert_eq!(
        line.iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ab", "name: ", "cd"]
    );
    assert!(line[1].is_ghost_text);
}

#[test]
fn test_render_data_adds_leading_space_before_eol_type_inlay_hint() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_inlay_hint(Cursor::new(0, 4), crate::buffer::Gravity::Right, " Type");

    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(1, 80));
    let line = render_data.get_line(0).expect("line should render");

    assert_eq!(
        line.iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["abcd", " Type"]
    );
    assert!(line[1].is_ghost_text);
}

#[test]
fn test_render_data_adds_trailing_space_after_eol_colon_inlay_hint() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_inlay_hint(Cursor::new(0, 4), crate::buffer::Gravity::Right, "name: ");

    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(1, 80));
    let line = render_data.get_line(0).expect("line should render");

    assert_eq!(
        line.iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["abcd", "name: "]
    );
}

#[test]
fn test_render_data_wraps_ghost_text_with_the_line() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "XY");

    let view = BufferView::new(buffer);
    let render_data = view.build_render_data_with_options(
        Size::new(2, 4),
        Style::default(),
        true,
        WrapMode::Hard,
        true,
    );

    assert_eq!(render_data.line_count(), 2);
    assert_eq!(
        render_data
            .get_line(0)
            .unwrap()
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ab", "XY"]
    );
    assert_eq!(
        render_data
            .get_line(1)
            .unwrap()
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>(),
        vec!["cd"]
    );
}

#[test]
fn test_render_data_out_of_bounds() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 80));

    assert!(render_data.get_line(10).is_none());
}

// Gutter tests
#[test]
fn test_gutter_width_calculation() {
    // 1-9 lines: 1 digit + 2 padding = 3 columns
    let gutter = Gutter::new(0, 10, 9);
    assert_eq!(gutter.calculate_width(), 3);

    // 1-99 lines: 2 digits + 2 padding = 4 columns
    let gutter = Gutter::new(0, 10, 99);
    assert_eq!(gutter.calculate_width(), 4);

    // 1-999 lines: 3 digits + 2 padding = 5 columns
    let gutter = Gutter::new(0, 10, 999);
    assert_eq!(gutter.calculate_width(), 5);

    // Empty buffer: minimum 3 columns
    let gutter = Gutter::new(0, 10, 0);
    assert_eq!(gutter.calculate_width(), 3);
}

#[test]
fn test_gutter_width_calculation_includes_diagnostic_sign_column() {
    let gutter = Gutter::new(0, 10, 9).with_diagnostic_sign_width(2);
    assert_eq!(gutter.calculate_width(), 5);
}

#[test]
fn test_gutter_width_calculation_includes_diff_sign_column() {
    let gutter = Gutter::new(0, 10, 9).with_diff_sign_width(1);
    assert_eq!(gutter.calculate_width(), 4);
}

#[test]
fn test_gutter_width_calculation_includes_fold_sign_column() {
    let gutter = Gutter::new(0, 10, 9).with_fold_sign_width(1);
    assert_eq!(gutter.calculate_width(), 4);
}

#[test]
fn test_gutter_render_for_render_data_uses_diagnostic_signs() {
    let buffer = Buffer::from_str("one\ntwo");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(2, 20));
    let mut gutter = Gutter::new(0, 2, 2).with_diagnostic_sign_width(1);
    let mut screen = crate::screen::Screen::new(2, 20);

    gutter.render_for_render_data(
        &mut screen,
        Position::new(0, 0),
        &render_data,
        GutterRenderState {
            cursor_line: 0,
            relative_number: false,
            active_screen_row: None,
            active_line_style: None,
            diff_markers: vec![None, None],
            diff_sign_width: 0,
            diff_added_sign_style: Style::default(),
            diff_deleted_sign_style: Style::default(),
            diff_modified_sign_style: Style::default(),
            diagnostic_severities: vec![
                Some(DiagnosticSeverity::ERROR),
                Some(DiagnosticSeverity::HINT),
            ],
            diagnostic_sign_width: 1,
            fold_sign_width: 0,
        },
    );

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "E");
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "H");
    let gutter_style = Style::new().bg(Color::ansi(236)).fg(Color::ansi(245));
    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        diagnostic_style_for(DiagnosticSeverity::ERROR, gutter_style)
    );
    assert_eq!(
        screen.get_cell_mut(1, 0).unwrap().style,
        diagnostic_style_for(DiagnosticSeverity::HINT, gutter_style)
    );
}

#[test]
fn test_gutter_render_for_render_data_uses_distinct_diff_signs() {
    let buffer = Buffer::from_str("one\ntwo\nthree");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 20));
    let mut gutter = Gutter::new(0, 3, 3).with_diff_sign_width(1);
    let mut screen = crate::screen::Screen::new(3, 20);

    gutter.render_for_render_data(
        &mut screen,
        Position::new(0, 0),
        &render_data,
        GutterRenderState {
            cursor_line: 0,
            relative_number: false,
            active_screen_row: None,
            active_line_style: None,
            diff_markers: vec![
                Some(DiffMarkerKind::Added),
                Some(DiffMarkerKind::Deleted),
                Some(DiffMarkerKind::Modified),
            ],
            diff_sign_width: 1,
            diff_added_sign_style: Style::new().fg(Color::ansi(10)),
            diff_deleted_sign_style: Style::new().fg(Color::ansi(9)),
            diff_modified_sign_style: Style::new().fg(Color::ansi(11)),
            diagnostic_severities: vec![None, None, None],
            diagnostic_sign_width: 0,
            fold_sign_width: 0,
        },
    );

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "+");
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "-");
    assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, "~");
    let gutter_style = Style::new().bg(Color::ansi(236)).fg(Color::ansi(245));
    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        gutter_style.overlay(Style::new().fg(Color::ansi(10)))
    );
    assert_eq!(
        screen.get_cell_mut(1, 0).unwrap().style,
        gutter_style.overlay(Style::new().fg(Color::ansi(9)))
    );
    assert_eq!(
        screen.get_cell_mut(2, 0).unwrap().style,
        gutter_style.overlay(Style::new().fg(Color::ansi(11)))
    );
}

#[test]
fn test_gutter_render_for_render_data_uses_nerdfont_diff_signs_when_enabled() {
    let _config_guard = globals::set_test_config(Config {
        advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
        ..Default::default()
    });
    let buffer = Buffer::from_str("one\ntwo\nthree");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 20));
    let mut gutter = Gutter::new(0, 3, 3).with_diff_sign_width(1);
    let mut screen = crate::screen::Screen::new(3, 20);

    gutter.render_for_render_data(
        &mut screen,
        Position::new(0, 0),
        &render_data,
        GutterRenderState {
            cursor_line: 0,
            relative_number: false,
            active_screen_row: None,
            active_line_style: None,
            diff_markers: vec![
                Some(DiffMarkerKind::Added),
                Some(DiffMarkerKind::Deleted),
                Some(DiffMarkerKind::Modified),
            ],
            diff_sign_width: 1,
            diff_added_sign_style: Style::default(),
            diff_deleted_sign_style: Style::default(),
            diff_modified_sign_style: Style::default(),
            diagnostic_severities: vec![None, None, None],
            diagnostic_sign_width: 0,
            fold_sign_width: 0,
        },
    );

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "");
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "");
    assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, "");
}

#[test]
fn test_gutter_render_for_render_data_uses_ascii_fold_glyphs() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner\nafter\ntail"));
    let mut screen = crate::screen::Screen::new(4, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "v");
    assert_eq!(screen.get_cell_mut(3, 3).unwrap().text, " ");

    window.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::new(ActionKind::CloseFold));
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, ">");
    assert_eq!(window.render_data().line_data[1].buffer_line, 2);
}

#[test]
fn test_gutter_render_only_marks_fold_starts() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner\nafter"));
    let mut screen = crate::screen::Screen::new(4, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "v");
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(2, 3).unwrap().text, " ");
}

#[test]
fn test_gutter_render_does_not_mark_blank_lines_as_fold_starts() {
    let mut window = Window::new(Buffer::from_str("outer\n  inner\n\n  nested\nafter"));
    let mut screen = crate::screen::Screen::new(5, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "v");
    assert_eq!(screen.get_cell_mut(2, 3).unwrap().text, " ");
}

#[test]
fn test_gutter_render_for_render_data_uses_unicode_fold_glyphs() {
    let _config_guard = globals::set_test_config(Config {
        advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::UnicodeFolds]),
        ..Default::default()
    });
    let mut window = Window::new(Buffer::from_str("outer\n  inner\nafter"));
    let mut screen = crate::screen::Screen::new(3, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "▼");

    window.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::new(ActionKind::CloseFold));
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 20));

    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "▶");
}

#[test]
fn test_window_render_applies_diff_markers_to_full_gutter_row() {
    let path = temp_path_with_ext("diff-gutter", "txt");
    let buffer = Buffer::from_str_with_path("one\ntwo\nthree", path);
    let mut window = Window::new(buffer);
    let buffer_id = window.buffer_view().buffer_id();
    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.apply_diff_refresh_result(DiffRefreshResult {
                buffer_id,
                generation: 0,
                tracked: true,
                hunks: vec![DiffHunk::new(1, 2)],
            });
        })
        .unwrap_or(());

    let mut screen = crate::screen::Screen::new(3, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 20));

    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "~");
}

#[test]
fn test_window_diff_hunk_navigation_moves_between_hunks() {
    let path = temp_path_with_ext("diff-hunks", "txt");
    let buffer = Buffer::from_str_with_path("one\ntwo\nthree\nfour\nfive", path);
    let mut window = Window::new(buffer);
    let buffer_id = window.buffer_view().buffer_id();
    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.apply_diff_refresh_result(DiffRefreshResult {
                buffer_id,
                generation: 0,
                tracked: true,
                hunks: vec![DiffHunk::new(1, 3), DiffHunk::new(4, 5)],
            });
        })
        .unwrap_or(());

    window.set_cursor(Cursor::new(0, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToNextDiffHunk)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 1);
    assert_eq!(window.buffer_view().cursor().col, 0);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToNextDiffHunk)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 4);
    assert_eq!(window.buffer_view().cursor().col, 0);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToPreviousDiffHunk)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 1);
    assert_eq!(window.buffer_view().cursor().col, 0);

    window.set_cursor(Cursor::new(2, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToNextDiffHunk)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 1);
    assert_eq!(window.buffer_view().cursor().col, 0);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToNextDiffHunkEnd)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 2);
    assert_eq!(window.buffer_view().cursor().col, 0);

    window.set_cursor(Cursor::new(1, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveToPreviousDiffHunkEnd)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 2);
    assert_eq!(window.buffer_view().cursor().col, 0);
}

#[test]
fn test_gutter_digit_count() {
    assert_eq!(Gutter::digit_count(0), 1);
    assert_eq!(Gutter::digit_count(9), 1);
    assert_eq!(Gutter::digit_count(10), 2);
    assert_eq!(Gutter::digit_count(99), 2);
    assert_eq!(Gutter::digit_count(100), 3);
    assert_eq!(Gutter::digit_count(999), 3);
    assert_eq!(Gutter::digit_count(1000), 4);
}

#[test]
fn test_window_render_supports_relative_line_numbers_in_all_modes() {
    let theme = syntax_themed_window();
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        relative_number: true,
        syntax: false,
        ..Default::default()
    });

    let expected_rows = [("2", 0), ("1", 1), ("3", 2), ("1", 3), ("2", 4)];
    for mode in [
        ModeKind::Normal,
        ModeKind::Insert,
        ModeKind::Visual,
        ModeKind::VisualLine,
    ] {
        let mut mode_window = Window::new(Buffer::from_str_with_path(
            "a\nb\nc\nd\ne",
            temp_path_with_ext(&format!("relative-number-{mode:?}"), "txt"),
        ));
        mode_window.switch_mode(mode);
        let mut screen = crate::screen::Screen::new(5, 20);
        mode_window.set_cursor(crate::buffer::Cursor::new(2, 0));
        mode_window.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

        for (expected, row) in expected_rows {
            assert_eq!(screen.get_cell_mut(row, 2).unwrap().text, expected);
        }
    }
}

#[test]
fn test_window_render_applies_active_gutter_style_to_full_row() {
    let path = temp_path_with_ext("active-gutter", "txt");
    let buffer = Buffer::from_str_with_path("a\nb\nc", path);
    let mut window = Window::new(buffer);
    let mut theme = syntax_themed_window();
    theme.highlights.insert(
        Tag::parse("ui.window.gutter.active_line").expect("valid tag"),
        Style::new().fg(Color::ansi(99)),
    );
    let _theme_guard = globals::set_test_active_theme(theme.clone());
    let _config_guard = globals::set_test_config(Config {
        active_line: true,
        relative_number: true,
        syntax: false,
        ..Default::default()
    });

    window.set_cursor(crate::buffer::Cursor::new(1, 0));

    let mut screen = crate::screen::Screen::new(3, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 20));

    let expected_active_style = theme
        .resolve_name_with_default("ui.window.gutter")
        .overlay(theme.highlight_style_for_name("ui.window.gutter.active_line"));
    let expected_base_style = theme.resolve_name_with_default("ui.window.gutter");
    assert_eq!(
        screen.get_cell_mut(1, 0).unwrap().style,
        expected_active_style
    );
    assert_eq!(
        screen.get_cell_mut(1, 1).unwrap().style,
        expected_active_style
    );
    assert_eq!(
        screen.get_cell_mut(1, 2).unwrap().style,
        expected_active_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        expected_base_style
    );
    assert_eq!(
        screen.get_cell_mut(2, 0).unwrap().style,
        expected_base_style
    );
}

#[test]
fn test_window_render_applies_diagnostic_undercurl_to_buffer_ranges() {
    let path = temp_path_with_ext("diagnostic-undercurl", "txt");
    let buffer = Buffer::from_str_with_path("abcd", path);
    let mut window = Window::new(buffer);
    let theme = syntax_themed_window();
    let _theme_guard = globals::set_test_active_theme(theme.clone());
    let _config_guard = globals::set_test_config(Config {
        syntax: false,
        ..Default::default()
    });

    let buffer_id = window.buffer_view().buffer_id();
    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "lsp-test",
            vec![Diagnostic {
                range: Range::new(
                    lsp_types::Position::new(0, 1),
                    lsp_types::Position::new(0, 3),
                ),
                severity: Some(DiagnosticSeverity::WARNING),
                code: None,
                code_description: None,
                source: Some("lsp".to_string()),
                message: "warning".to_string(),
                related_information: None,
                tags: None,
                data: None,
            }],
        );
    });

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let expected_base_style = theme.default_style();
    let expected_diagnostic_style = expected_base_style.overlay(diagnostic_undercurl_style_for(
        DiagnosticSeverity::WARNING,
        Style::default(),
    ));
    let content_col = Gutter::new(0, 1, 1)
        .with_diagnostic_sign_width(1)
        .with_fold_sign_width(FOLD_SIGN_WIDTH)
        .calculate_width();

    assert_eq!(
        screen.get_cell_mut(0, content_col).unwrap().style,
        expected_base_style
    );
    assert_eq!(
        screen.get_cell_mut(0, content_col + 1).unwrap().style,
        expected_diagnostic_style
    );
    assert_eq!(
        screen.get_cell_mut(0, content_col + 2).unwrap().style,
        expected_diagnostic_style
    );
    assert_eq!(
        screen.get_cell_mut(0, content_col + 3).unwrap().style,
        expected_base_style
    );
}

#[test]
fn test_gutter_render_background() {
    // Use 10 lines so gutter width is 4 (digits(10) + 2 = 4)
    let mut gutter = Gutter::new(0, 5, 10);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    let gutter_width = gutter.calculate_width();
    assert_eq!(gutter_width, 4); // Verify expected width

    // Check background is rendered for all visible rows in gutter area
    for row in 0..5 {
        for col in 0..gutter_width {
            let _cell = screen.get_cell_mut(row, col).unwrap();
            // Most cells should be spaces (background or padding)
            // Only specific columns should have line numbers
        }
    }

    // Specifically check that gutter cells have spaces (not line numbers)
    // Column 0 should always be space (left padding)
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
}

#[test]
fn test_gutter_render_line_numbers() {
    // For 10 lines: digits(10) + 2 = 4 columns
    // Layout: col0=left_pad, col1=empty/1st_digit, col2=2nd_digit/last_digit, col3=right_pad
    let mut gutter = Gutter::new(0, 3, 10);
    let mut screen = crate::screen::Screen::new(3, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Width is digits(10) + 2 = 4
    // Line "1": col0=space, col1=space, col2="1", col3=space
    let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
    assert_eq!(cell_left_pad.text, " "); // left padding
    let cell_empty = screen.get_cell_mut(0, 1).unwrap();
    assert_eq!(cell_empty.text, " "); // empty for 1-digit
    let cell_num = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_num.text, "1"); // line number right-aligned
    let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
    assert_eq!(cell_right_pad.text, " "); // right padding

    // Line "2": col0=space, col1=space, col2="2", col3=space
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 2).unwrap().text, "2");
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, " ");

    // Line "3": col0=space, col1=space, col2="3", col3=space
    assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(2, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(2, 2).unwrap().text, "3");
    assert_eq!(screen.get_cell_mut(2, 3).unwrap().text, " ");
}

#[test]
fn test_gutter_wrap_detection() {
    // Simulate scrolling where same buffer line appears in multiple screen rows
    // start_line=5, visible_rows=2 would show buffer lines 5 and 6
    // With 10 lines: width = 4
    // Row 0: buffer line 5 -> "6" at column 2, right padding at column 3
    // Row 1: buffer line 6 -> "7" at column 2, right padding at column 3
    let mut gutter = Gutter::new(5, 2, 10);
    let mut screen = crate::screen::Screen::new(2, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Row 0: buffer line 5 -> "6" (1-indexed)
    // Line "6" at column 2 (right-aligned for 1-digit)
    let cell_0 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_0.text, "6");

    // Row 1: buffer line 6 -> "7" (1-indexed)
    let cell_1 = screen.get_cell_mut(1, 2).unwrap();
    assert_eq!(cell_1.text, "7");
}

#[test]
fn test_gutter_scroll_offset() {
    // Test gutter with scroll offset
    // With 20 total lines: digits(20) + 2 = 4 columns
    // start_line=10 means first visible is buffer line 10 (display 11, 2 digits)
    let mut gutter = Gutter::new(10, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Verify gutter width
    assert_eq!(gutter.calculate_width(), 4);

    // First visible line is buffer line 10 (1-indexed: 11, 2 digits)
    // Layout: col0=left_pad, col1="1", col2="1", col3=right_pad
    let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
    assert_eq!(cell_left_pad.text, " "); // left padding
    let cell_digit1 = screen.get_cell_mut(0, 1).unwrap();
    assert_eq!(cell_digit1.text, "1"); // first digit of "11"
    let cell_digit2 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_digit2.text, "1"); // second digit of "11"
    let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
    assert_eq!(cell_right_pad.text, " "); // right padding
}

#[test]
fn test_window_visual_cursor_with_gutter() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let mut window = Window::new(buffer);

    // Set cursor to line 0, column 2 (within "line1")
    window.buffer_view_mut().set_cursor(Cursor::new(0, 2));

    // Need to call render to build render_data first
    let size = Size::new(3, 80);
    let mut screen = crate::screen::Screen::new(3, 80);
    window.render(&mut screen, Position::new(0, 0), size);

    // Get visual cursor position
    let cursor_pos = window.visual_cursor();

    assert!(cursor_pos.is_some());
    let pos = cursor_pos.unwrap();

    // Cursor should be offset by gutter width (5 columns for 3 lines plus fold sign spacing)
    // The cursor is at column 2 in the content, plus 5 for gutter = column 7
    let gutter_width = 5; // digits(3) + 2 + fold sign + spacer = 5
    assert_eq!(pos.col, 2 + gutter_width);
}

#[test]
fn test_window_visual_cursor_ignores_ghost_text() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "ghost");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 3));

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let pos = window.visual_cursor().expect("cursor should be visible");
    assert_eq!(pos.col, 13);
}

#[test]
fn test_window_visual_cursor_does_not_count_ghost_text_at_cursor() {
    let mut buffer = Buffer::from_str("abcd");
    buffer.insert_ghost_text(Cursor::new(0, 2), crate::buffer::Gravity::Right, "ghost");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 2));

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let pos = window.visual_cursor().expect("cursor should be visible");
    assert_eq!(pos.col, 12);
}

#[test]
fn test_render_data_cursor_ignores_ghost_text_after_overlay_split() {
    let mut render_data = RenderData::new(1);
    render_data.line_data.push(LineData {
        buffer_line: 0,
        byte_offset: 0,
        end_byte: 4,
        width_offset: 0,
        show_gutter_line_number: true,
        base_style: Style::default(),
        fold_glyph: None,
        folded_line_count: None,
        chunks: vec![
            RenderChunk::new("ab", Style::default()),
            RenderChunk::ghost_text("ghost", Style::default()),
            RenderChunk::new("cd", Style::default()),
        ],
    });

    render_data.accent_range(
        Cursor::new(0, 0),
        Cursor::new(0, 4),
        Style::default().fg(Color::ansi(1)),
    );

    let ghost_text = render_data.line_data[0]
        .chunks
        .iter()
        .filter(|chunk| chunk.is_ghost_text)
        .map(|chunk| chunk.text.as_str())
        .collect::<String>();
    assert_eq!(ghost_text, "ghost");
    assert_eq!(
        render_data.cursor_screen_position(Cursor::new(0, 3)),
        Some(Position::new(0, 8))
    );
}

#[test]
fn test_render_data_accent_range_ignores_ghost_text_when_counting_bytes() {
    let mut render_data = RenderData::new(1);
    render_data.line_data.push(LineData {
        buffer_line: 0,
        byte_offset: 0,
        end_byte: 4,
        width_offset: 0,
        show_gutter_line_number: true,
        base_style: Style::default(),
        fold_glyph: None,
        folded_line_count: None,
        chunks: vec![
            RenderChunk::new("ab", Style::default()),
            RenderChunk::ghost_text("ghost", Style::default()),
            RenderChunk::new("cd", Style::default()),
        ],
    });

    render_data.accent_range(
        Cursor::new(0, 3),
        Cursor::new(0, 4),
        Style::default().fg(Color::ansi(1)),
    );

    let styles = render_data.line_data[0]
        .chunks
        .iter()
        .map(|chunk| (chunk.text.as_str(), chunk.style))
        .collect::<Vec<_>>();

    assert_eq!(
        styles,
        vec![
            ("ab", Style::default()),
            ("ghost", Style::default()),
            ("c", Style::default()),
            ("d", Style::default().fg(Color::ansi(1))),
        ]
    );
}

#[test]
fn test_window_visual_cursor_drops_deleted_inlay_hint_after_di_paren() {
    let mut buffer = Buffer::from_str("    let _guard = urvim::logger::init(\"debug.log\");");
    buffer.insert_inlay_hint(
        Cursor::new(0, 14),
        crate::buffer::Gravity::Right,
        ": WorkerGuard",
    );
    buffer.insert_inlay_hint(
        Cursor::new(0, 37),
        crate::buffer::Gravity::Right,
        "log_file: ",
    );
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(0, 36));

    assert_eq!(
        window.dispatch_action(&Action::operation(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
        )),
        ActionResult::Handled
    );

    let mut screen = crate::screen::Screen::new(3, 136);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 136));
    let pos = window.visual_cursor().expect("cursor should be visible");
    let rendered_text = window
        .render_data()
        .get_line(0)
        .expect("line should render")
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<String>();

    assert_eq!(
        buffer_text(window.buffer_view()),
        "    let _guard = urvim::logger::init();"
    );
    assert!(rendered_text.contains(": WorkerGuard"));
    assert!(!rendered_text.contains("log_file"));
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 37));
    assert_eq!(pos.col, 55);
}

#[test]
fn test_toggle_wrap_action_toggles_window_state() {
    let mut window = Window::new(Buffer::from_str("line"));
    assert!(!window.wrap_enabled());

    window.toggle_wrap();
    assert!(window.wrap_enabled());

    window.toggle_wrap();
    assert!(!window.wrap_enabled());
}

#[test]
fn test_build_render_data_wraps_hard_mode() {
    let view = BufferView::new(Buffer::from_str("abcdefgh"));
    let render_data = view.build_render_data_with_options(
        Size::new(3, 4),
        Style::default(),
        true,
        WrapMode::Hard,
        true,
    );

    assert_eq!(render_data.line_count(), 2);
    assert_eq!(render_data.get_line(0).unwrap()[0].text, "abcd");
    assert_eq!(render_data.get_line(1).unwrap()[0].text, "efgh");
}

#[test]
fn test_build_render_data_wraps_soft_mode_at_word_boundary() {
    let view = BufferView::new(Buffer::from_str("hello world"));
    let render_data = view.build_render_data_with_options(
        Size::new(3, 6),
        Style::default(),
        true,
        WrapMode::Soft,
        true,
    );

    assert_eq!(render_data.line_count(), 2);
    assert_eq!(render_data.get_line(0).unwrap()[0].text, "hello ");
    assert_eq!(render_data.get_line(1).unwrap()[0].text, "world");
}

#[test]
fn test_build_render_data_soft_wrap_falls_back_to_hard_break() {
    let view = BufferView::new(Buffer::from_str("superlongword"));
    let render_data = view.build_render_data_with_options(
        Size::new(4, 4),
        Style::default(),
        true,
        WrapMode::Soft,
        true,
    );

    assert_eq!(render_data.get_line(0).unwrap()[0].text, "supe");
    assert_eq!(render_data.get_line(1).unwrap()[0].text, "rlon");
}

#[test]
fn test_window_render_hides_gutter_line_number_on_wrapped_continuation() {
    let _config_guard = globals::set_test_config(Config {
        wrap_mode: WrapMode::Hard,
        ..Default::default()
    });
    let mut window = Window::new(Buffer::from_str("abcdefghij"));
    window.set_wrap_enabled(true);
    let mut screen = crate::screen::Screen::new(2, 9);

    window.render(&mut screen, Position::new(0, 0), Size::new(2, 9));

    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(1, 1).unwrap().text, " ");
}

#[test]
fn test_window_visual_cursor_maps_to_wrapped_continuation_row() {
    let _config_guard = globals::set_test_config(Config {
        wrap_mode: WrapMode::Hard,
        ..Default::default()
    });
    let mut window = Window::new(Buffer::from_str("abcdefghi"));
    window.set_wrap_enabled(true);
    window.set_cursor(Cursor::new(0, 5));
    let mut screen = crate::screen::Screen::new(3, 7);

    window.render(&mut screen, Position::new(0, 0), Size::new(3, 7));

    let cursor = window.visual_cursor().expect("cursor should be visible");
    assert_eq!(cursor.row, 1);
    assert_eq!(cursor.col, 6);
}

#[test]
fn test_vertical_motions_remain_logical_lines_when_wrapping_is_enabled() {
    let _config_guard = globals::set_test_config(Config {
        wrap_mode: WrapMode::Hard,
        ..Default::default()
    });
    let mut window = Window::new(Buffer::from_str("abcdefghij\nxy\nklmnop"));
    window.set_wrap_enabled(true);
    window.set_cursor(Cursor::new(0, 6));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveDown)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 1);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveDown)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 2);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveUp)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor().line, 1);
}

#[test]
fn test_wrapped_eof_cursor_stays_in_viewport_and_reveals_overflow_rows() {
    let _config_guard = globals::set_test_config(Config {
        wrap_mode: WrapMode::Hard,
        ..Default::default()
    });
    // With content width 4:
    // - line 1 wraps into 2 rows (1 overflow)
    // - line 2 wraps into 4 rows (3 overflow)
    let mut window = Window::new(Buffer::from_str("abcdefgh\nabcdefghijklmnop"));
    window.set_wrap_enabled(true);
    window.set_cursor(Cursor::new(1, 15));
    let mut screen = crate::screen::Screen::new(3, 7);

    window.render(&mut screen, Position::new(0, 0), Size::new(3, 7));

    let cursor = window
        .visual_cursor()
        .expect("cursor should remain visible");
    assert!(cursor.row < 3);
    // Reaching EOF in wrapped mode should scroll into line-2 continuations.
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
}

#[test]
fn test_visual_selection_is_rendered() {
    let mut theme = themed_window();
    theme.highlights.insert(
        Tag::parse("ui.selection").expect("valid tag"),
        Style::new().bg(Color::ansi(99)),
    );
    let expected_style = theme.highlight_style_for_name("ui.selection");
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let line = rendered_line(&window, 0);
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "ab" && chunk.style == expected_style)
    );
}

#[test]
fn test_visual_line_selection_is_rendered() {
    let mut theme = themed_window();
    theme.highlights.insert(
        Tag::parse("ui.selection").expect("valid tag"),
        Style::new().bg(Color::ansi(99)),
    );
    let expected_style = theme.highlight_style_for_name("ui.selection");
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let buffer = Buffer::from_str("abc\ndef");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 1));

    let mut screen = crate::screen::Screen::new(2, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(2, 20));

    let first = rendered_line(&window, 0);
    assert!(
        first
            .iter()
            .any(|chunk| chunk.text == "abc" && chunk.style == expected_style)
    );
    let second = rendered_line(&window, 1);
    assert!(
        second
            .iter()
            .any(|chunk| chunk.text == "def" && chunk.style == expected_style)
    );
}

#[test]
fn test_normal_yank_characterwise_flashes_selection() {
    let (_t, _c) = visual_test_setup();
    let expected_style = themed_window().highlight_style_for_name("ui.selection");

    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));

    let action = Action::operation(
        Operator::Yank,
        OperatorTarget::TextObject(TextObject::InnerWord),
    );
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let line = rendered_line(&window, 0);
    assert!(
        line.iter()
            .any(|chunk| chunk.text == "hello" && chunk.style == expected_style)
    );
}

#[test]
fn test_normal_yank_line_flashes_selection() {
    let (_t, _c) = visual_test_setup();
    let expected_style = themed_window().highlight_style_for_name("ui.selection");

    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::YankLine)),
        ActionResult::Handled
    );

    let mut screen = crate::screen::Screen::new(2, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(2, 20));

    let first = rendered_line(&window, 0);
    assert!(
        first
            .iter()
            .any(|chunk| chunk.text == "alpha" && chunk.style == expected_style)
    );
    let second = rendered_line(&window, 1);
    assert!(!second.iter().any(|chunk| chunk.style == expected_style));
}

#[test]
fn test_normal_counted_linewise_yank_motion_flashes_selection() {
    let (_t, _c) = visual_test_setup();
    let expected_style = themed_window().highlight_style_for_name("ui.selection");

    let buffer = Buffer::from_str("one\ntwo\nthree\nfour");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(3, 0));

    // Equivalent to y2gg in Vim: yank from the current line to line 2.
    let action = Action::count(
        2,
        Box::new(Action::operation(
            Operator::Yank,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        )),
    );
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    let mut screen = crate::screen::Screen::new(4, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    // Yank should include lines 2-4 (0-based lines 1..=3).
    for line_idx in 1..=3 {
        let line = rendered_line(&window, line_idx);
        assert!(
            line.iter().any(|chunk| chunk.style == expected_style),
            "expected yank flash on line {}",
            line_idx
        );
    }

    let first = rendered_line(&window, 0);
    assert!(!first.iter().any(|chunk| chunk.style == expected_style));
}

#[test]
fn test_normal_yank_restarts_flash_on_subsequent_yank() {
    let (_t, _c) = visual_test_setup();
    let expected_style = themed_window().highlight_style_for_name("ui.selection");

    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::YankLine)),
        ActionResult::Handled
    );
    window.buffer_view_mut().set_cursor(Cursor::new(1, 0));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::YankLine)),
        ActionResult::Handled
    );

    let mut screen = crate::screen::Screen::new(2, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(2, 20));

    let first = rendered_line(&window, 0);
    assert!(!first.iter().any(|chunk| chunk.style == expected_style));
    let second = rendered_line(&window, 1);
    assert!(
        second
            .iter()
            .any(|chunk| chunk.text == "beta" && chunk.style == expected_style)
    );
}

#[test]
fn test_normal_yank_flash_expires_and_is_cleared_on_render() {
    let (_t, _c) = visual_test_setup();
    let expected_style = themed_window().highlight_style_for_name("ui.selection");

    let buffer = Buffer::from_str("alpha");
    let mut window = Window::new(buffer);
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::YankLine)),
        ActionResult::Handled
    );

    thread::sleep(Duration::from_millis(220));

    let mut screen = crate::screen::Screen::new(1, 20);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 20));

    let line = rendered_line(&window, 0);
    assert!(!line.iter().any(|chunk| chunk.style == expected_style));
}

#[test]
fn test_visual_repeated_motion_matches_counted_motion() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("abcdef");
    let mut counted = Window::new(buffer.clone());
    counted
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    let counted_action = Action::new(ActionKind::MoveRight)
        .with_count(3)
        .expect("counted motion should be allowed")
        .with_from_mode(ModeKind::Visual);
    assert_eq!(
        counted.dispatch_action(&counted_action),
        ActionResult::Handled
    );

    let mut repeated = Window::new(buffer);
    repeated
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    let motion = Action::new(ActionKind::MoveRight).with_from_mode(ModeKind::Visual);
    for _ in 0..3 {
        assert_eq!(repeated.dispatch_action(&motion), ActionResult::Handled);
    }

    assert_eq!(
        counted.buffer_view().cursor(),
        repeated.buffer_view().cursor()
    );
    assert_eq!(
        counted.buffer_view().visual_selection_range(),
        repeated.buffer_view().visual_selection_range()
    );
}

#[test]
fn test_visual_delete_leaves_cursor_at_selection_start() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::DeleteSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "c");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_visual_change_leaves_cursor_at_selection_start() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::ChangeSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Insert);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "c");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_visual_change_undo_restores_original_text() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::ChangeSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Insert);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "c");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("x".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "xc");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 1));

    commit_insert_exit_snapshot(&mut window);

    apply_undo(&mut window);

    assert_eq!(buffer_text(window.buffer_view()), "abc");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_visual_text_object_repeats_are_idempotent() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);

    let action = Action::new(ActionKind::VisualTextObject(TextObject::InnerWord))
        .with_from_mode(ModeKind::Visual);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    let expected = Some(crate::buffer::TextObjectRange {
        start: Cursor::new(0, 0),
        end: Cursor::new(0, 5),
    });
    assert_eq!(window.buffer_view().visual_selection_range(), expected);
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));

    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(window.buffer_view().visual_selection_range(), expected);
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
}

#[test]
fn test_visual_text_object_can_retarget_selection() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);

    let inner_word = Action::new(ActionKind::VisualTextObject(TextObject::InnerWord))
        .with_from_mode(ModeKind::Visual);
    assert_eq!(window.dispatch_action(&inner_word), ActionResult::Handled);

    let around_word = Action::new(ActionKind::VisualTextObject(TextObject::AroundWord))
        .with_from_mode(ModeKind::Visual);
    assert_eq!(window.dispatch_action(&around_word), ActionResult::Handled);
    assert_eq!(
        window.buffer_view().visual_selection_range(),
        Some(crate::buffer::TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 6),
        })
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 5));
}

#[test]
fn test_visual_text_object_invalid_location_leaves_selection_unchanged() {
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);

    let before = window.buffer_view().visual_selection_range();
    let action = Action::new(ActionKind::VisualTextObject(TextObject::InnerBracket(
        BracketKind::Paren,
    )))
    .with_from_mode(ModeKind::Visual);

    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(window.buffer_view().visual_selection_range(), before);
}

#[test]
fn test_visual_case_lowercases_selection_and_exits_to_normal() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("AbC");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 3));

    let action = Action::operation(Operator::Lowercase, OperatorTarget::Selection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "abc");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
    assert_eq!(window.mode_kind(), ModeKind::Normal);
}

#[test]
fn test_case_uppercase_operator_handles_unicode_expansion() {
    let buffer = Buffer::from_str("straße");
    let mut window = Window::new(buffer);

    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Uppercase,
        OperatorTarget::TextObject(TextObject::InnerBigWord),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "STRASSE");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_visual_line_toggle_case_handles_unicode_and_exits_to_normal() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("foo\nßa");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 0));

    let action = Action::operation(Operator::ToggleCase, OperatorTarget::Selection)
        .with_from_mode(ModeKind::VisualLine)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "FOO\nSSA");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
    assert_eq!(window.mode_kind(), ModeKind::Normal);
}

#[test]
fn test_visual_yank_copies_selection_without_mutating_buffer() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));

    let action = Action::new(ActionKind::YankSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "abc");

    let content = globals::with_register_store(|store| store.get(RegisterName('y')))
        .expect("register store should be available");
    assert_eq!(content.text, "a");
    assert_eq!(content.kind, RegisterContentKind::Characterwise);
}

#[test]
fn test_visual_line_delete_removes_entire_lines() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("one\ntwo\nthree\nfour");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(2, 1));

    let action = Action::new(ActionKind::DeleteSelection)
        .with_from_mode(ModeKind::VisualLine)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "one\nfour");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
}

#[test]
fn test_visual_line_change_leaves_blank_line() {
    let (_t, _c) = visual_test_setup();

    let buffer = Buffer::from_str("one\ntwo\nthree\nfour");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(2, 1));

    let action = Action::new(ActionKind::ChangeSelection)
        .with_from_mode(ModeKind::VisualLine)
        .with_to_mode(ModeKind::Insert);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "one\n\nfour");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
}

#[test]
fn test_visual_line_yank_copies_lines_without_mutating_buffer() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour");
    let mut window = Window::new(buffer);
    window.buffer_view_mut().set_cursor(Cursor::new(1, 1));
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Line);
    window.buffer_view_mut().set_cursor(Cursor::new(2, 1));

    let action = Action::new(ActionKind::YankSelection)
        .with_from_mode(ModeKind::VisualLine)
        .with_to_mode(ModeKind::Normal);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo\nthree\nfour");

    let content = globals::with_register_store(|store| store.get(RegisterName('y')))
        .expect("register store should be available");
    assert_eq!(content.text, "two\nthree");
    assert_eq!(content.kind, RegisterContentKind::Linewise);
}

#[test]
fn test_gutter_scroll_and_rerender() {
    // Simulate scrolling and re-rendering
    // First render at start_line=0
    let mut gutter = Gutter::new(0, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Verify initial render - line 1 should have gutter style
    // For 20 lines, width = digits(20) + 2 = 2 + 2 = 4
    // Line "1" (digit 1): col0=space, col1=space, col2="1", col3=space
    let cell_line1 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_line1.text, "1");

    // Now simulate scrolling - create new gutter at start_line=3
    let mut gutter2 = Gutter::new(3, 5, 20);
    let mut screen2 = crate::screen::Screen::new(5, 80);

    gutter2.render(&mut screen2, Position::new(0, 0));

    // After scrolling to line 3, row 0 should show line 4 (buffer line 3 + 1)
    // Line "4": col0=space, col1=space, col2="4", col3=space
    let cell_scrolled = screen2.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_scrolled.text, "4");

    // Verify gutter background is rendered for ALL rows including empty ones
    // Row 4 would be buffer line 7 which doesn't exist in 20 lines, but background should still be there
    let cell_empty_row = screen2.get_cell_mut(4, 0).unwrap();
    assert_eq!(cell_empty_row.text, " ");
}

#[test]
fn test_gutter_then_buffer_render() {
    // Test that buffer content doesn't overwrite gutter
    // This simulates what happens in Window::render
    let gutter_width = 4; // digits(20) + 2 = 4

    // First render gutter
    let mut gutter = Gutter::new(0, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);
    gutter.render(&mut screen, Position::new(0, 0));

    // Verify gutter cells have correct content
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // Now simulate buffer content rendering at offset
    let content_origin = Position::new(0, gutter_width);
    let content_size = Size::new(5, 80 - gutter_width);

    // Create some buffer content to render
    let buffer = crate::buffer::Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(content_size);
    render_data.render(&mut screen, content_origin, content_size, Style::default());

    // After buffer rendering, gutter cells should STILL have correct gutter content
    // Gutter is at columns 0-3, buffer is at column 4+
    // Column 0 should still be gutter left padding
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    // Column 2 should still have line number "1" (not overwritten by buffer)
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    // Column 3 should still be gutter right padding
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // But buffer content should be at column 4+
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, "l"); // "line1"
}

#[test]
fn test_gutter_width_change() {
    // Test gutter when width changes (e.g., file grows from 99 to 100 lines)
    // Old gutter width = 4 (digits(99) + 2 = 2 + 2)
    // New gutter width = 5 (digits(100) + 2 = 3 + 2)

    // Simulate first render with width=4
    let mut screen = crate::screen::Screen::new(3, 80);
    let mut gutter = Gutter::new(0, 3, 99);
    gutter.render(&mut screen, Position::new(0, 0));

    // With width=4 and line "1":
    // col0=space, col1=space, col2="1", col3=space
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // Now simulate re-render with width=5 (simulating file grew)
    // The screen still has old content, but we re-render with new width
    let mut gutter2 = Gutter::new(0, 3, 100);
    gutter2.render(&mut screen, Position::new(0, 0));

    // With width=5 and line "1" (1 digit):
    // col0=space, col1=space, col2=space, col3="1", col4=space
    // Because: right_padding at col4, line at col4-1=3
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");

    // Also verify multi-digit line number
    // Line "11" would be at columns 2-3
    let mut gutter3 = Gutter::new(9, 3, 100); // start at line 9, showing 10, 11
    gutter3.render(&mut screen, Position::new(0, 0));

    // Line "10" at row 0: col2="1", col3="0", col4=space
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "0");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");
}

// Column preservation tests

#[test]
fn test_column_preservation_first_vertical_move() {
    // First vertical move should use current column and remember it
    let buffer = Buffer::from_str("abcdefgh\nij");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // First move down via Window - should use current column (5), remember it
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor().line, 1);
    // Line 2 is "ij", so normal mode should clamp to its last character.
    assert_eq!(window.buffer_view.cursor().col, 1);
}

#[test]
fn test_column_preservation_consecutive_vertical_moves() {
    // Consecutive vertical moves should preserve remembered column
    let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down - remembers column 5
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

    // Move down again - should use remembered column 5
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));

    // Move up - should use remembered column 5
    window.dispatch_action(&Action::new(ActionKind::MoveUp));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));
}

#[test]
fn test_column_preservation_horizontal_resets() {
    // Horizontal movement should reset remembered column
    use crate::editor::Action;

    let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down - remembers column 5
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

    // Move right - should reset remembered column to current (now at column 6)
    window.dispatch_action(&Action::new(ActionKind::MoveRight));
    // Now at column 6 on line 1

    // Move down again - should use new column 6 and go to line 2
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 6));
}

#[test]
fn test_column_preservation_clamp_on_short_line() {
    // Moving to shorter line should clamp to end of line
    let buffer = Buffer::from_str("abcdefgh\nij\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down to shorter line "ij" (length 2)
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    // Should clamp to column 1 (last character of "ij")
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 1));

    // Move down to longer line - should use remembered column 5
    window.dispatch_action(&Action::new(ActionKind::MoveDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));
}

#[test]
fn test_normal_cursor_does_not_move_past_last_character() {
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(0, 1));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveRight)),
        ActionResult::Handled
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveRight)),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
}

#[test]
fn test_visual_cursor_does_not_move_past_last_character() {
    let (_t, _c) = visual_test_setup();
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Visual);
    window.set_cursor(Cursor::new(0, 1));

    let action = Action::new(ActionKind::MoveRight).with_from_mode(ModeKind::Visual);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
}

#[test]
fn test_normal_cursor_does_not_move_before_line_start() {
    let buffer = Buffer::from_str("abc\ndef");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(1, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveLeft)),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
}

#[test]
fn test_visual_cursor_does_not_move_before_line_start() {
    let (_t, _c) = visual_test_setup();
    let buffer = Buffer::from_str("abc\ndef");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Visual);
    window.set_cursor(Cursor::new(1, 0));

    let action = Action::new(ActionKind::MoveLeft).with_from_mode(ModeKind::Visual);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
}

#[test]
fn test_replace_cursor_does_not_move_before_line_start() {
    let buffer = Buffer::from_str("abc\ndef");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Replace);
    window.set_cursor(Cursor::new(1, 0));

    let action = Action::new(ActionKind::MoveLeft).with_from_mode(ModeKind::Replace);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 0));
}

#[test]
fn test_insert_cursor_can_move_to_previous_line_end() {
    let buffer = Buffer::from_str("abc\ndef");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Insert);
    window.set_cursor(Cursor::new(1, 0));

    let action = Action::new(ActionKind::MoveLeft).with_from_mode(ModeKind::Insert);
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 3));
}

#[test]
fn test_leaving_insert_clamps_cursor_to_last_character() {
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Insert);
    window.set_cursor(Cursor::new(0, 3));

    window.switch_mode(ModeKind::Normal);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 2));
}

#[test]
fn test_non_insert_cursor_can_remain_on_empty_line() {
    let buffer = Buffer::from_str("");
    let mut window = Window::new(buffer);
    window.set_cursor(Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::MoveRight)),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_mode_cursor_can_stay_at_line_end() {
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window.switch_mode(ModeKind::Insert);
    window.set_cursor(Cursor::new(0, 3));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_char('d')
                .with_from_mode(ModeKind::Insert)
                .with_to_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 4));
}

#[test]
fn test_action_resets_remembered_column() {
    use crate::buffer::Boundary;
    use crate::editor::Action;

    // Horizontal movements should reset
    assert!(Action::new(ActionKind::MoveLeft).resets_remembered_column());
    assert!(Action::new(ActionKind::MoveRight).resets_remembered_column());
    assert!(Action::forward_to(Boundary::Word).resets_remembered_column());
    assert!(Action::back_to(Boundary::Word).resets_remembered_column());
    assert!(Action::new(ActionKind::MoveToLineEnd).resets_remembered_column());
    assert!(Action::new(ActionKind::MoveToLineStart).resets_remembered_column());
    assert!(Action::new(ActionKind::MoveToLineContentStart).resets_remembered_column());

    // Vertical movements should NOT reset
    assert!(!Action::new(ActionKind::MoveUp).resets_remembered_column());
    assert!(!Action::new(ActionKind::MoveDown).resets_remembered_column());

    // Other actions should not reset
    assert!(!Action::mode_transition(ModeKind::Insert).resets_remembered_column());
    assert!(Action::insert_char('a').resets_remembered_column());
    assert!(Action::new(ActionKind::DeleteBackward).resets_remembered_column());
    assert!(Action::new(ActionKind::DeleteForward).resets_remembered_column());
}

#[test]
fn test_action_uses_remembered_column() {
    use crate::editor::Action;

    // Vertical movements should use remembered column
    assert!(Action::new(ActionKind::MoveUp).uses_remembered_column());
    assert!(Action::new(ActionKind::MoveDown).uses_remembered_column());
    assert!(Action::new(ActionKind::MovePageUp).uses_remembered_column());
    assert!(Action::new(ActionKind::MovePageDown).uses_remembered_column());
    assert!(Action::new(ActionKind::MoveHalfPageUp).uses_remembered_column());
    assert!(Action::new(ActionKind::MoveHalfPageDown).uses_remembered_column());

    // Other movements should NOT
    assert!(!Action::new(ActionKind::MoveLeft).uses_remembered_column());
    assert!(!Action::new(ActionKind::MoveRight).uses_remembered_column());
}

#[test]
fn test_action_page_motions_do_not_reset_remembered_column() {
    assert!(!Action::new(ActionKind::MovePageUp).resets_remembered_column());
    assert!(!Action::new(ActionKind::MovePageDown).resets_remembered_column());
    assert!(!Action::new(ActionKind::MoveHalfPageUp).resets_remembered_column());
    assert!(!Action::new(ActionKind::MoveHalfPageDown).resets_remembered_column());
}

#[test]
fn test_action_page_motions_update_snapshot_cursor() {
    assert!(Action::new(ActionKind::MovePageUp).updates_snapshot_cursor());
    assert!(Action::new(ActionKind::MovePageDown).updates_snapshot_cursor());
    assert!(Action::new(ActionKind::MoveHalfPageUp).updates_snapshot_cursor());
    assert!(Action::new(ActionKind::MoveHalfPageDown).updates_snapshot_cursor());
}

#[test]
fn test_page_motions_move_by_viewport_height() {
    let buffer = Buffer::from_str("0123456789\nabcdefghij\nklmnopqrst\nuvwxyz0123");
    let mut window = Window::new(buffer);
    window.buffer_view.set_cursor(Cursor::new(0, 8));

    let mut screen = crate::screen::Screen::new(3, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    window.dispatch_action(&Action::new(ActionKind::MovePageDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(3, 8));

    window.dispatch_action(&Action::new(ActionKind::MovePageUp));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 8));
}

#[test]
fn test_page_motions_render_updated_gutter_line_numbers() {
    let buffer = Buffer::from_str("line 1\nline 2\nline 3\nline 4\nline 5\nline 6");
    let mut window = Window::new(buffer);
    let size = Size::new(3, 40);
    let gutter_col = 1;

    let mut screen = crate::screen::Screen::new(3, 40);
    window.render(&mut screen, Position::new(0, 0), size);
    assert_eq!(screen.get_cell_mut(0, gutter_col).unwrap().text, "1");

    window.dispatch_action(&Action::new(ActionKind::MovePageDown));
    window.render(&mut screen, Position::new(0, 0), size);
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(2, 0));
    assert_eq!(screen.get_cell_mut(0, gutter_col).unwrap().text, "3");
    assert_eq!(screen.get_cell_mut(1, gutter_col).unwrap().text, "4");
    assert_eq!(screen.get_cell_mut(2, gutter_col).unwrap().text, "5");

    window.dispatch_action(&Action::new(ActionKind::MovePageUp));
    window.render(&mut screen, Position::new(0, 0), size);
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(0, 0));
    assert_eq!(screen.get_cell_mut(0, gutter_col).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(1, gutter_col).unwrap().text, "2");
    assert_eq!(screen.get_cell_mut(2, gutter_col).unwrap().text, "3");
}

#[test]
fn test_page_motions_clamp_on_short_line() {
    let buffer = Buffer::from_str("0123456789\nabcdefghij\nklmnopqrst\nuv");
    let mut window = Window::new(buffer);
    window.buffer_view.set_cursor(Cursor::new(0, 8));

    let mut screen = crate::screen::Screen::new(3, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    window.dispatch_action(&Action::new(ActionKind::MovePageDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(3, 1));
}

#[test]
fn test_half_page_motions_move_by_half_viewport_height() {
    let buffer = Buffer::from_str("0123456789\nabcdefghij\nklmnopqrst\nuvwxyz0123");
    let mut window = Window::new(buffer);
    window.buffer_view.set_cursor(Cursor::new(0, 8));

    let mut screen = crate::screen::Screen::new(3, 40);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    window.dispatch_action(&Action::new(ActionKind::MoveHalfPageDown));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 8));

    window.dispatch_action(&Action::new(ActionKind::MoveHalfPageUp));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 8));
}

// Character Scan Motion Tests

#[test]
fn test_find_forward_moves_to_char() {
    // "hello world" - cursor at 'h', find 'o'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.dispatch_action(&Action::find_forward('o'));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4)); // 'o' is at column 4
}

#[test]
fn test_find_forward_finds_third_occurrence() {
    // "x x x" - find 3rd 'x'
    let buffer = Buffer::from_str("x x x");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::count(3, Box::new(Action::find_forward('x'))));
    // Third 'x' is at column 4
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_find_forward_not_found_stays_in_place() {
    // "hello" - find 'z' (doesn't exist)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2)); // at 'l'
    window.dispatch_action(&Action::find_forward('z'));
    // Cursor should stay at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_find_backward_moves_to_char() {
    // "hello world" - cursor at 'd', find 'o' (first when going backward from cursor)
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10)); // at 'd'
    window.dispatch_action(&Action::find_backward('o'));
    // First 'o' when going backward from position 10 is at column 7
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 7));
}

#[test]
fn test_find_backward_not_found_stays_in_place() {
    // "hello" - cursor at 'h', find 'x' (doesn't exist before)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.dispatch_action(&Action::find_backward('x'));
    // Cursor should stay at column 0
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_till_forward_lands_before_char() {
    // "hello" - cursor at 'h', till 'o' should land on 'l' (column 3)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.dispatch_action(&Action::till_forward('o'));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 3)); // 'l' is at column 3
}

#[test]
fn test_till_forward_clamp_at_line_start() {
    // "hello" - cursor at 'h', till 'h' should clamp to column 0
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.dispatch_action(&Action::till_forward('h'));
    // Till lands one before 'h', which would be column -1, clamped to 0
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_till_backward_lands_after_char() {
    // "hello" - cursor at 'l', till 'e' should land on 'e' (column 1)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 4)); // at 'o'
    window.dispatch_action(&Action::till_backward('h'));
    // Till backward 'h' from 'o': 'h' is at 0, +1 = column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_backward_clamp_at_line_end() {
    // "hello" - cursor at 'o', till 'o' - no previous 'o' to find, so stays
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 4)); // at 'o'
    window.dispatch_action(&Action::till_backward('o'));
    // Till backward 'o' from 'o': there's no 'o' before position 4, so cursor stays
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_find_forward_with_count() {
    // "hello world" - 2fx finds 2nd 'o'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    // Use Count wrapper for the action
    window.dispatch_action(&Action::count(2, Box::new(Action::find_forward('o'))));
    // 'o' appears at column 4 and 7
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 7)); // second 'o'
}

#[test]
fn test_find_backward_with_count() {
    // "hello world" - 2Fl finds 2nd 'l' when going backward from 'd'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10)); // at 'd'
    window.dispatch_action(&Action::count(2, Box::new(Action::find_backward('l'))));
    // 'l' appears at columns 2, 3, and 9
    // Going backward from 'd' at 10: 1st 'l' is at 9, 2nd 'l' is at 3
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 3));
}

#[test]
fn test_find_backward_skips_current_char_on_duplicate() {
    // "helllo" - cursor on 3rd 'l', Fl should find 2nd 'l'
    let buffer = Buffer::from_str("helllo");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 3)); // at 3rd 'l'
    window.dispatch_action(&Action::find_backward('l'));
    // Should find 2nd 'l' at column 2, not 3rd 'l' at column 3
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_delete_character_scan_operator_updates_repeat_state() {
    let buffer = Buffer::from_str("foo:bar");
    let mut window = Window::new(buffer);
    let expected = FindState {
        target_char: ':',
        kind: FindKind::Find,
        direction: Direction::Forward,
    };

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::CharacterScan(expected),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "bar");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
    assert_eq!(globals::get_last_find(), Some(expected));
}

#[test]
fn test_counted_character_scan_operator_uses_motion_count() {
    let buffer = Buffer::from_str("foo:bar:baz");
    let mut window = Window::new(buffer);
    let expected = FindState {
        target_char: ':',
        kind: FindKind::Find,
        direction: Direction::Forward,
    };

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::count(
        2,
        Box::new(Action::operation(
            Operator::Delete,
            OperatorTarget::CharacterScan(expected),
        )),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "baz");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
    assert_eq!(globals::get_last_find(), Some(expected));
}

#[test]
fn test_count_diw_deletes_multiple_words() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::count(
        3,
        Box::new(Action::operation(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerWord),
        )),
    ));

    assert_eq!(
        window
            .buffer_view
            .with_buffer(|buffer| buffer.line_at(0).map(|line| line.to_string()))
            .flatten(),
        Some(" four".to_string())
    );
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dw_deletes_through_next_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_cw_changes_through_next_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
    assert!(
        Action::operation(
            Operator::Change,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward)
        )
        .with_to_mode(ModeKind::Insert)
        .switches_to_insert_mode()
    );
}

#[test]
fn test_ciw_keeps_syntax_styled_above_changed_line() {
    let _lock = syntax_worker_lock();
    let _theme_guard = globals::set_test_active_theme(syntax_themed_window());
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let value = String::new();\n}}\n{body}");
    let path = temp_path_with_ext("cw-render-fallback", "rs");
    let buffer = Buffer::from_str_with_path(&source, path);
    let mut window = Window::new(buffer);

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1))
        })
        .unwrap();

    let edit_line = window
        .buffer_view()
        .with_buffer(|buffer| {
            (0..buffer.line_count())
                .find(|line| {
                    buffer
                        .line_at(*line)
                        .is_some_and(|line_text| line_text.to_string().contains("let value"))
                })
                .expect("value line should exist")
        })
        .unwrap();
    let line_text = window
        .buffer_view()
        .with_buffer(|buffer| buffer.line_at(edit_line).map(|line| line.to_string()))
        .flatten()
        .expect("value line should exist");
    let value_start = line_text.find("value").expect("line should contain value");

    window
        .buffer_view_mut()
        .set_cursor(Cursor::new(edit_line, value_start));
    assert_eq!(
        window.dispatch_action(&Action::operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerWord),
        )),
        ActionResult::Handled
    );
    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("foo".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );

    window
        .buffer_view_mut()
        .set_scroll_offset(Position::new(edit_line.saturating_sub(1) as u16, 0));

    let mut screen = crate::screen::Screen::new(4, 120);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 120));

    let expected_keyword_style = Style::new().fg(Color::ansi(23));
    let line_above = rendered_line(&window, 0);
    assert!(line_above.iter().any(|chunk| chunk.text == "fn"));

    let edited_line = rendered_line(&window, 1);
    assert!(
        edited_line
            .iter()
            .any(|chunk| chunk.text == "let" && chunk.style == expected_keyword_style)
    );
    assert!(
        edited_line
            .iter()
            .any(|chunk| chunk.text == "foo" && chunk.style == Style::new().fg(Color::ansi(29)))
    );
    assert!(
        edited_line
            .iter()
            .any(|chunk| chunk.text == "String" && chunk.style == Style::new().fg(Color::ansi(28)))
    );
}

#[test]
fn test_open_line_after_ciw_keeps_prefix_styled() {
    let _lock = syntax_worker_lock();
    let _theme_guard = globals::set_test_active_theme(syntax_themed_window());
    let _config_guard = globals::set_test_config(Config {
        theme: "demo-syntax".to_string(),
        insert_escape: None,
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let source = "fn main() {\n    let value = String::new();\n}\nfn helper() {}";
    let path = temp_path_with_ext("open-line-after-ciw", "rs");
    let buffer = Buffer::from_str_with_path(source, path);
    let mut window = Window::new(buffer);

    window
        .buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1))
        })
        .unwrap();

    let edit_line = 1;
    let line_text = window
        .buffer_view()
        .with_buffer(|buffer| buffer.line_at(edit_line).map(|line| line.to_string()))
        .flatten()
        .expect("value line should exist");
    let value_start = line_text.find("value").expect("line should contain value");

    window
        .buffer_view_mut()
        .set_cursor(Cursor::new(edit_line, value_start));
    assert_eq!(
        window.dispatch_action(&Action::operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerWord),
        )),
        ActionResult::Handled
    );
    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("foo".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(
        window.dispatch_action(
            &Action::new(ActionKind::OpenLineBelow).with_to_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );

    window
        .buffer_view_mut()
        .set_scroll_offset(Position::new(0, 0));
    let mut screen = crate::screen::Screen::new(4, 120);
    window.render(&mut screen, Position::new(0, 0), Size::new(4, 120));

    let first = rendered_line(&window, 0);
    assert!(first.iter().any(|chunk| chunk.text == "fn"));
    let second = rendered_line(&window, 1);
    assert!(second.iter().any(|chunk| chunk.text == "let"));
}

#[test]
fn test_cw_undo_restores_original_text() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(
        &mut window,
        &Action::operation(
            Operator::Change,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        ),
    );

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    assert_eq!(
        window.dispatch_action(
            &Action::insert_text("hi".to_string()).with_from_mode(ModeKind::Insert)
        ),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hiworld");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));

    commit_insert_exit_snapshot(&mut window);

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "hello world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_cw_at_end_of_line_is_noop() {
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5));
    let result = window.dispatch_action(&Action::operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(result, ActionResult::NotHandled);
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 5));
}

#[test]
fn test_da_paren_deletes_around_bracket_pair() {
    let buffer = Buffer::from_str("foo(bar)baz");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 4));
    let result = window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::TextObject(TextObject::AroundBracket(BracketKind::Paren)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foobaz");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 3));
}

#[test]
fn test_di_quote_deletes_inner_quote_pair() {
    let buffer = Buffer::from_str("foo \"bar\" baz");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    let result = window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo \"\" baz");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 5));
}

#[test]
fn test_di_quote_with_no_matching_pair_is_noop() {
    let buffer = Buffer::from_str("foo bar");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Single)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo bar");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_di_paren_with_no_matching_pair_is_noop() {
    let buffer = Buffer::from_str("foo bar");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "foo bar");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_di_paren_on_empty_pair_is_noop() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_ci_paren_on_empty_pair_enters_insert_point() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Change,
        OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_ci_quote_on_empty_pair_enters_insert_point() {
    let buffer = Buffer::from_str("\"\"");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Change,
        OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "\"\"");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_insert_text_auto_closes_supported_pair_in_insert_mode() {
    let buffer = Buffer::from_str("");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::insert_char('(').with_from_mode(ModeKind::Insert));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_insert_char_skips_supported_closer_in_insert_mode() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let result = window.dispatch_action(&Action::insert_char(')').with_from_mode(ModeKind::Insert));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_insert_char_skips_quote_closer_in_insert_mode() {
    let buffer = Buffer::from_str("\"\"");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let result = window.dispatch_action(&Action::insert_char('"').with_from_mode(ModeKind::Insert));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "\"\"");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_delete_backward_removes_supported_pair_in_insert_mode() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let result = window
        .dispatch_action(&Action::new(ActionKind::DeleteBackward).with_from_mode(ModeKind::Insert));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_pairing_disabled_keeps_plain_insert_and_delete_behavior() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(false));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let insert_result =
        window.dispatch_action(&Action::insert_char(')').with_from_mode(ModeKind::Insert));
    assert_eq!(insert_result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "())");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let delete_result = window
        .dispatch_action(&Action::new(ActionKind::DeleteBackward).with_from_mode(ModeKind::Insert));

    assert_eq!(delete_result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "))");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_insert_pair_undo_and_redo_restore_exact_states() {
    let buffer = Buffer::from_str("");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.dispatch_action(&Action::insert_char('(').with_from_mode(ModeKind::Insert));
    assert_eq!(result, ActionResult::Handled);
    let cursor = window.buffer_view.cursor();
    window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
        .unwrap_or(());

    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_redo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_pair_delete_undo_and_redo_restore_exact_states() {
    let buffer = Buffer::from_str("()");
    let mut window = Window::new(buffer);
    let _config_guard = globals::set_test_config(pairing_test_config(true));

    window.buffer_view.set_cursor(Cursor::new(0, 1));
    let result = window
        .dispatch_action(&Action::new(ActionKind::DeleteBackward).with_from_mode(ModeKind::Insert));
    assert_eq!(result, ActionResult::Handled);
    let cursor = window.buffer_view.cursor();
    window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
        .unwrap_or(());

    assert_eq!(buffer_text(window.buffer_view()), "");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "()");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_redo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_set_cursor_synced_normalizes_stored_cursor_after_buffer_change() {
    let buffer = Buffer::from_str("a😀b");
    let mut window = Window::new(buffer);

    window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.remove(Cursor::new(0, 0), Cursor::new(0, 1)))
        .unwrap_or(());

    window.set_cursor_synced(Cursor::new(0, 3));

    assert_eq!(buffer_text(window.buffer_view()), "😀b");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_delete_forward_undo_and_redo() {
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(&mut window, &Action::new(ActionKind::DeleteForward));

    assert_eq!(buffer_text(window.buffer_view()), "ello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_redo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "ello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dw_undo_and_redo() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(
        &mut window,
        &Action::operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        ),
    );

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "hello world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    apply_redo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_cg_changes_to_first_line() {
    let buffer = Buffer::from_str("one\ntwo\nthree");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(1, 0));
    let result = window.dispatch_action(&Action::operation(
        Operator::Change,
        OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "one");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_counted_dw_undo_restores_original_text() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(
        &mut window,
        &Action::count(
            2,
            Box::new(Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            )),
        ),
    );

    assert_eq!(buffer_text(window.buffer_view()), "three four");

    apply_undo(&mut window);
    assert_eq!(buffer_text(window.buffer_view()), "one two three four");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dollar_deletes_to_line_end() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "hello ");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 6));
}

#[test]
fn test_d0_deletes_to_line_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dcaret_deletes_to_line_content_start() {
    let buffer = Buffer::from_str("    hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "    world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_db_deletes_back_to_previous_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dgg_deletes_to_first_line_linewise() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(3, 1));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "five");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_d_g_deletes_to_last_line_linewise() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(2, 0));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 0));
}

#[test]
fn test_counted_d_g_deletes_to_destination_line() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive\nsix");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(2, 0));
    window.dispatch_action(&Action::count(
        5,
        Box::new(Action::operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
        )),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo\nsix");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 0));
}

#[test]
fn test_dw_with_count_deletes_multiple_words() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::count(
        2,
        Box::new(Action::operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "three four");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dbigword_forward_and_backward() {
    let buffer = Buffer::from_str("alpha --- beta");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
    ));
    assert_eq!(buffer_text(window.buffer_view()), "--- beta");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    let buffer = Buffer::from_str("alpha --- beta");
    let mut window = Window::new(buffer);
    window.buffer_view.set_cursor(Cursor::new(0, 10));
    window.dispatch_action(&Action::operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
    ));
    assert_eq!(buffer_text(window.buffer_view()), "alpha beta");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 6));
}

#[test]
fn test_till_forward_repeated_finds_next_occurrence() {
    // "hello" - tl repeated should find subsequent 'l's
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.dispatch_action(&Action::till_forward('l'));
    // First 'l' at column 2, land before it at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));

    window.dispatch_action(&Action::till_forward('l'));
    // Second 'l' at column 3, land before it at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_till_backward_repeated_finds_previous_occurrence() {
    // "hhello" - Th repeated should find previous 'h's
    let buffer = Buffer::from_str("hhello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5)); // at 'o'
    window.dispatch_action(&Action::till_backward('h'));
    // First 'h' at column 1, land after it at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));

    window.dispatch_action(&Action::till_backward('h'));
    // Second 'h' at column 0, land after it at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));

    window.dispatch_action(&Action::till_backward('h'));
    // No more 'h' before column 0, cursor stays at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_forward_preserves_grapheme_boundaries() {
    let buffer = Buffer::from_str("a😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.dispatch_action(&Action::till_forward('b'));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_backward_preserves_grapheme_boundaries() {
    let buffer = Buffer::from_str("a😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5));
    window.dispatch_action(&Action::till_backward('a'));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_move_to_last_line_preserves_visual_column() {
    let buffer = Buffer::from_str("ab\na😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.dispatch_action(&Action::new(ActionKind::MoveToLastLine));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 1));
}

#[test]
fn test_count_screen_top_preserves_visual_column() {
    let buffer = Buffer::from_str("ab\na😀b\ncd");
    let mut window = Window::new(buffer);

    window.size = Size::new(2, 10);
    window.buffer_view.set_scroll_offset(Position::new(1, 0));
    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.dispatch_action(&Action::count(
        1,
        Box::new(Action::new(ActionKind::MoveToScreenTop)),
    ));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 1));
}

#[test]
fn test_viewport_cursor_alignment_repositions_without_moving_cursor() {
    let buffer = Buffer::from_str("0\n1\n2\n3\n4\n5\n6\n7\n8\n9");
    let mut window = Window::new(buffer);

    window.size = Size::new(5, 20);
    window.buffer_view.set_cursor(Cursor::new(6, 0));
    window.buffer_view.set_scroll_offset(Position::new(0, 3));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorTop));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(5, 3));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(6, 0));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorCenter));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(4, 3));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(6, 0));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorBottom));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(2, 3));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(6, 0));
}

#[test]
fn test_viewport_cursor_alignment_clamps_near_buffer_end() {
    let buffer = Buffer::from_str("0\n1\n2\n3\n4\n5\n6\n7");
    let mut window = Window::new(buffer);

    window.size = Size::new(5, 20);
    window.buffer_view.set_cursor(Cursor::new(7, 0));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorTop));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(3, 0));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(7, 0));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorCenter));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(3, 0));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(7, 0));

    window.dispatch_action(&Action::new(ActionKind::ViewportCursorBottom));
    assert_eq!(window.buffer_view.scroll_offset(), Position::new(3, 0));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(7, 0));
}

#[test]
fn test_next_paragraph_clamps_visual_column_on_blank_line() {
    let buffer = Buffer::from_str("ab\n\ncd");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.dispatch_action(&Action::new(ActionKind::MoveToNextParagraph));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 0));
}

#[test]
fn test_toggle_line_comment_uses_active_syntax_prefix() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-window", "rs").as_path())
        .unwrap();
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut window = Window::new(buffer);

    let result = window.dispatch_action(&Action::toggle_line_comment());

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.line_at(0).map(|line| line.to_string()))
            .unwrap(),
        Some("// fn main() {}".to_string())
    );
}

#[test]
fn test_toggle_line_comment_count_applies_to_multiple_lines() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-count", "rs").as_path())
        .unwrap();
    let buffer = Buffer::from_str_with_path("fn a() {}\nfn b() {}\nfn c() {}", path);
    let mut window = Window::new(buffer);

    let result = window.dispatch_action(&Action::count(3, Box::new(Action::toggle_line_comment())));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.as_str().to_string())
            .unwrap(),
        "// fn a() {}\n// fn b() {}\n// fn c() {}".to_string()
    );
}

#[test]
fn test_toggle_line_comment_aligns_to_minimum_column_across_range() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-align", "rs").as_path())
        .unwrap();
    let buffer = Buffer::from_str_with_path("  fn a() {}\n    fn b() {}", path);
    let mut window = Window::new(buffer);

    let result = window.dispatch_action(&Action::count(2, Box::new(Action::toggle_line_comment())));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.as_str().to_string())
            .unwrap(),
        "  // fn a() {}\n  //   fn b() {}".to_string()
    );
}

#[test]
fn test_toggle_line_comment_skips_blank_lines() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-blank", "py").as_path())
        .unwrap();
    let buffer = Buffer::from_str_with_path("\n    print('hello')", path);
    let mut window = Window::new(buffer);

    let result = window.dispatch_action(&Action::count(2, Box::new(Action::toggle_line_comment())));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.as_str().to_string())
            .unwrap(),
        "\n    # print('hello')".to_string()
    );
}

#[test]
fn test_toggle_line_comment_returns_not_handled_without_prefix() {
    let buffer = Buffer::from_str("plain text");
    let mut window = Window::new(buffer);

    let result = window.dispatch_action(&Action::toggle_line_comment());

    assert_eq!(result, ActionResult::NotHandled);
    assert_eq!(
        window
            .buffer_view()
            .with_buffer(|buffer| buffer.as_str().to_string())
            .unwrap(),
        "plain text".to_string()
    );
}

#[test]
fn test_yank_line_populates_yank_register_and_paste_after_uses_it() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::YankLine)),
        ActionResult::Handled
    );

    let content = globals::with_register_store(|store| store.get(RegisterName('y')))
        .expect("register store should be available");
    assert_eq!(content.text, "alpha");
    assert_eq!(content.kind, RegisterContentKind::Linewise);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "alpha\nalpha\nbeta".to_string()
    );
}

#[test]
fn test_visual_yank_uses_explicit_named_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("abc");
    let mut window = Window::new(buffer);
    window
        .buffer_view_mut()
        .begin_visual_selection(VisualSelectionKind::Character);
    window.buffer_view_mut().set_cursor(Cursor::new(0, 0));

    let action = Action::new(ActionKind::YankSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Normal)
        .with_register(RegisterName('z'));
    assert_eq!(window.dispatch_action(&action), ActionResult::Handled);

    let content = globals::with_register_store(|store| store.get(RegisterName('z')))
        .expect("register store should be available");
    assert_eq!(content.text, "a");
    assert_eq!(content.kind, RegisterContentKind::Characterwise);
}

#[test]
fn test_delete_line_uses_configured_default_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let _config_guard = globals::set_test_config(Config {
        default_registers: DefaultRegisters {
            yank: 'y',
            delete: 'n',
            change: 'c',
        },
        ..pairing_test_config(true)
    });
    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::new(ActionKind::DeleteLine)),
        ActionResult::Handled
    );

    let content = globals::with_register_store(|store| store.get(RegisterName('n')))
        .expect("register store should be available");
    assert_eq!(content.text, "alpha");
    assert_eq!(content.kind, RegisterContentKind::Linewise);
    assert_eq!(buffer_text(window.buffer_view()), "beta".to_string());
}

#[test]
fn test_paste_after_uses_explicit_named_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('z'),
            RegisterContent::new("hi".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after().with_register(RegisterName('z'))),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hiab".to_string());
}

// ── Paste cursor-position regression tests ───────────────────────────────

#[test]
fn paste_after_characterwise_puts_cursor_at_end_of_pasted_text() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hello".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "helloab");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 5));
}

#[test]
fn paste_before_characterwise_puts_cursor_at_start_of_pasted_text() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hello".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_before()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "helloab");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn paste_after_characterwise_multiline_puts_cursor_at_end() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hi\nthere".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hi\nthereab");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 5));
}

#[test]
fn paste_after_linewise_puts_cursor_at_end_of_last_pasted_line() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hello".to_string(), RegisterContentKind::Linewise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "ab\nhello");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(1, 4));
}

#[test]
fn paste_before_linewise_puts_cursor_at_start_of_first_pasted_line() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hello".to_string(), RegisterContentKind::Linewise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_before()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hello\nab");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn paste_after_linewise_multiline_puts_cursor_at_end_of_last_line() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("one\ntwo".to_string(), RegisterContentKind::Linewise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "ab\none\ntwo");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(2, 2));
}

#[test]
fn paste_before_linewise_multiline_puts_cursor_at_start_of_first_line() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("one\ntwo".to_string(), RegisterContentKind::Linewise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_before()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo\nab");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn paste_after_characterwise_inserts_at_cursor_not_after_character() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new("hello".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(buffer_text(window.buffer_view()), "hellohello world");
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 5));
}

#[test]
fn paste_after_linewise_multiple_lines_inserts_all_content() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new(
                "one\ntwo\nthree\nfour".to_string(),
                RegisterContentKind::Linewise,
            ),
        );
    });

    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_after()),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "alpha\none\ntwo\nthree\nfour\nbeta"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(4, 3));
}

#[test]
fn paste_before_linewise_multiple_lines_inserts_all_content() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName('y'),
            RegisterContent::new(
                "one\ntwo\nthree\nfour".to_string(),
                RegisterContentKind::Linewise,
            ),
        );
    });

    let buffer = Buffer::from_str("alpha\nbeta");
    let mut window = Window::new(buffer);

    assert_eq!(
        window.dispatch_action(&Action::paste_before()),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_text(window.buffer_view()),
        "one\ntwo\nthree\nfour\nalpha\nbeta"
    );
    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_indent_guide_appears_with_step_by_step_editing_at_column_zero() {
    // Start with flat, non-nested content (all at indent 0).
    // This creates no valid scopes in the cache (all are invalidated single-line).
    let buffer = Buffer::from_str("a\nb");
    let mut window = Window::new(buffer);

    let _config_guard = globals::set_test_config(Config {
        indent_guides: true,
        ..Default::default()
    });

    // NO warmup render - simulate the app state when user opens a file
    // and immediately starts editing. The caches are empty.

    // Simulate typing character by character as the user would:
    // Press `o` on line 1 (cursor at end of "b") to open a new line
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(1, 1), '\n'))
        .unwrap();
    // Now buffer: ["a", "b", ""], cursor goes to line 2

    // Type `{` on the new empty line
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(2, 0), '{'))
        .unwrap();
    // Buffer: ["a", "b", "{"]

    // Press Enter to split the line
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(2, 1), '\n'))
        .unwrap();
    // Buffer: ["a", "b", "{", ""]

    // Type spaces (user presses space 3 times for indentation)
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(3, 0), ' '))
        .unwrap();
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(3, 1), ' '))
        .unwrap();
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(3, 2), ' '))
        .unwrap();

    // Type `"hello"`
    for ch in "\"hello\"".chars() {
        window
            .buffer_view_mut()
            .with_buffer_mut(|b| {
                b.insert_char(
                    Cursor::new(3, b.line_at(3).map(|l| l.len()).unwrap_or(0)),
                    ch,
                )
            })
            .unwrap();
    }

    // Press Enter to split
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| {
            b.insert_char(
                Cursor::new(3, b.line_at(3).map(|l| l.len()).unwrap_or(0)),
                '\n',
            )
        })
        .unwrap();

    // Type `}`
    window
        .buffer_view_mut()
        .with_buffer_mut(|b| b.insert_char(Cursor::new(4, 0), '}'))
        .unwrap();
    // Buffer: ["a", "b", "{", "   \"hello\"", "}"]

    // Cursor is on the indented line (line 3 = `   "hello"`)
    window.set_cursor(Cursor::new(3, 3));

    let mut screen = crate::screen::Screen::new(5, 24);
    window.render(&mut screen, Position::new(0, 0), Size::new(5, 24));

    // Check for the guide character on screen row for the `"hello"` line.
    // Screen layout: 5 rows (0-4), gutter width includes the reserved fold sign.
    // Row 3 = buffer line 3 = `   "hello"`; guide is at the first content column.
    let guide_cell = screen
        .get_cell_mut(3, 5)
        .map(|c| c.text.clone())
        .unwrap_or_default();
    assert_eq!(
        guide_cell, "|",
        "indent guide should appear on the indented line after step-by-step edit"
    );
}
