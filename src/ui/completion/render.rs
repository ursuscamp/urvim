//! Rendering helpers for the completion popup.

use super::CompletionCandidate;
use crate::screen::Screen;
use crate::terminal::Style;
use crate::ui::line_format::{
    EllipsisPlacement, FormattedLineSection, FormattedLineSegment, FormattedLineTemplate,
    LineSectionAlignment, LineSectionOverflow,
};
use crate::{globals, icon};
use lsp_types::CompletionItemKind;
use unicode_width::UnicodeWidthStr;

pub(super) fn completion_selection_prefix() -> String {
    icon::selection_prefix()
}

pub(super) fn completion_item_display_width(item: &CompletionCandidate) -> usize {
    let prefix_width = UnicodeWidthStr::width(completion_selection_prefix().as_str());
    let mut width = prefix_width;

    if item.kind.is_some() {
        width = width.saturating_add(UnicodeWidthStr::width(
            completion_item_kind_badge(item.kind.unwrap()).as_str(),
        ));
    } else if let Some(symbol) = item.symbol.as_ref() {
        width = width.saturating_add(UnicodeWidthStr::width(symbol.as_str()));
    }

    width = width.saturating_add(UnicodeWidthStr::width(item.label.as_str()));
    width = width.saturating_add(5);
    width = width.saturating_add(UnicodeWidthStr::width(
        completion_item_label_detail_text(item).as_str(),
    ));
    width = width.saturating_add(UnicodeWidthStr::width(
        completion_item_right_detail_text(item).as_str(),
    ));
    width
}

pub(super) fn completion_row_segments(
    item: &CompletionCandidate,
    style: Style,
    prefix: String,
    available_cols: u16,
) -> Vec<FormattedLineSegment> {
    let base_style = completion_item_base_style(item, style);
    let kind_style = completion_item_kind_style(item, base_style);
    let label_detail_style = completion_item_label_detail_style(item, base_style);
    let right_detail_style = completion_item_right_detail_style(item, base_style);
    let label_style = base_style;
    let prefix_width = UnicodeWidthStr::width(completion_selection_prefix().as_str()) as u16;

    let kind_text = item
        .kind
        .map(completion_item_kind_badge)
        .or_else(|| item.symbol.clone())
        .unwrap_or_default();
    let label_detail_text = completion_item_label_detail_text(item);
    let right_detail_text = completion_item_right_detail_text(item);

    let sections = vec![
        FormattedLineSection::fixed(prefix_width, base_style),
        FormattedLineSection::measured(kind_style),
        FormattedLineSection::fixed(1, base_style),
        FormattedLineSection::measured(label_style)
            .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::End)),
        FormattedLineSection::fixed(5, base_style),
        FormattedLineSection::measured(label_detail_style),
        FormattedLineSection::flex(1, base_style),
        FormattedLineSection::measured(right_detail_style)
            .with_alignment(LineSectionAlignment::Right)
            .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
    ];

    FormattedLineTemplate::new(sections)
        .render_segments(
            vec![
                prefix,
                kind_text,
                String::new(),
                item.label.clone(),
                String::new(),
                label_detail_text,
                String::new(),
                right_detail_text,
            ],
            available_cols,
        )
        .unwrap_or_default()
}

pub(super) fn completion_item_base_style(item: &CompletionCandidate, style: Style) -> Style {
    if item.deprecated {
        style
            .accent(theme_style("syntax.comment"))
            .faint()
            .italic()
            .strikethrough()
    } else {
        style
    }
}

pub(super) fn completion_item_kind_style(item: &CompletionCandidate, style: Style) -> Style {
    if let Some(kind) = item.kind {
        style.accent(completion_item_kind_style_for_kind(kind))
    } else {
        style
    }
}

pub(super) fn completion_item_label_detail_style(
    item: &CompletionCandidate,
    style: Style,
) -> Style {
    if item.deprecated {
        style.faint().italic()
    } else {
        style.faint().italic()
    }
}

pub(super) fn completion_item_right_detail_style(
    item: &CompletionCandidate,
    style: Style,
) -> Style {
    if item.deprecated {
        style.faint()
    } else {
        style.faint()
    }
}

pub(super) fn completion_item_kind_badge(kind: CompletionItemKind) -> String {
    icon::completion_item_kind_badge(kind)
}

fn completion_item_kind_style_for_kind(kind: CompletionItemKind) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(completion_item_kind_style_name(kind)))
            .unwrap_or_default()
    })
}

fn completion_item_kind_style_name(kind: CompletionItemKind) -> &'static str {
    match kind {
        CompletionItemKind::FILE | CompletionItemKind::MODULE | CompletionItemKind::FOLDER => {
            "syntax.namespace"
        }
        CompletionItemKind::CLASS
        | CompletionItemKind::STRUCT
        | CompletionItemKind::INTERFACE
        | CompletionItemKind::ENUM
        | CompletionItemKind::TYPE_PARAMETER => "syntax.type",
        CompletionItemKind::FUNCTION
        | CompletionItemKind::METHOD
        | CompletionItemKind::CONSTRUCTOR => "syntax.function",
        CompletionItemKind::PROPERTY | CompletionItemKind::FIELD | CompletionItemKind::VARIABLE => {
            "syntax.variable"
        }
        CompletionItemKind::CONSTANT
        | CompletionItemKind::ENUM_MEMBER
        | CompletionItemKind::VALUE => "syntax.constant",
        CompletionItemKind::UNIT | CompletionItemKind::COLOR => "syntax.number",
        CompletionItemKind::KEYWORD => "syntax.keyword",
        CompletionItemKind::OPERATOR => "syntax.operator",
        CompletionItemKind::TEXT
        | CompletionItemKind::REFERENCE
        | CompletionItemKind::SNIPPET
        | CompletionItemKind::EVENT => "syntax.variable",
        _ => "syntax.variable",
    }
}

fn completion_item_label_detail_text(item: &CompletionCandidate) -> String {
    let mut parts = Vec::new();
    if let Some(label_detail) = item
        .label_detail
        .as_ref()
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(label_detail.as_str());
    }
    if let Some(label_description) = item
        .label_description
        .as_ref()
        .filter(|text| !text.trim().is_empty())
    {
        parts.push(label_description.as_str());
    }
    match parts.as_slice() {
        [] => String::new(),
        [single] => format!(" {single}"),
        [first, second, ..] => format!(" {first} · {second}"),
    }
}

fn completion_item_right_detail_text(item: &CompletionCandidate) -> String {
    item.detail
        .as_ref()
        .filter(|text| !text.trim().is_empty())
        .map(|detail| format!(" {detail}"))
        .unwrap_or_default()
}

pub(super) fn theme_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

pub(super) fn render_segments(
    screen: &mut Screen,
    row: u16,
    col: u16,
    segments: Vec<FormattedLineSegment>,
) {
    let mut current_col = col;
    for segment in segments {
        screen.write_string(row, current_col, segment.style, segment.text.as_str());
        current_col += UnicodeWidthStr::width(segment.text.as_str()) as u16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{Cursor, TextObjectRange};
    use crate::config::{AdvancedGlyphCapability, Config};
    use crate::globals;
    use crate::terminal::Color;
    use crate::theme::{HighlightStyles, Tag, Theme, ThemeKind};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn completion_uses_nerdfont_selection_prefix_when_enabled() {
        let _guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        assert_eq!(completion_selection_prefix(), " ");
    }

    #[test]
    fn completion_uses_ascii_selection_prefix_when_nerdfonts_are_disabled() {
        let _guard = globals::set_test_config(Config::default());

        assert_eq!(completion_selection_prefix(), "> ");
    }

    #[test]
    fn completion_kind_style_only_applies_to_the_kind_badge() {
        let mut highlights = BTreeMap::new();
        highlights.insert(
            Tag::parse("syntax.comment").expect("valid tag"),
            Style::new().fg(Color::ansi(245)),
        );
        highlights.insert(
            Tag::parse("syntax.function").expect("valid tag"),
            Style::new().fg(Color::ansi(196)).bold(),
        );
        let _theme_guard = globals::set_test_active_theme(Theme::new(
            "test",
            ThemeKind::Ansi256,
            Style::new(),
            HighlightStyles::new(highlights),
        ));
        let _guard = globals::set_test_config(Config::default());
        let item = CompletionCandidate {
            label: "String::new".to_string(),
            replacement: "String::new()".to_string(),
            range: TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 11),
            },
            symbol: None,
            kind: Some(CompletionItemKind::FUNCTION),
            insert_format: None,
            detail: None,
            label_detail: None,
            label_description: None,
            additional_text_edits: Vec::new(),
            lsp_completion_item: None,
            deprecated: false,
            preselect: false,
        };

        let base_style = Style::new().bg(Color::ansi(17));
        let segments = completion_row_segments(&item, base_style, "> ".to_string(), 40);

        assert_eq!(segments[0].style, base_style);
        assert_eq!(segments[2].style, base_style);
        assert_eq!(
            segments[1].style,
            base_style.accent(Style::new().fg(Color::ansi(196)).bold())
        );
    }

    #[test]
    fn deprecated_completion_items_use_the_muted_theme_style() {
        let mut highlights = BTreeMap::new();
        highlights.insert(
            Tag::parse("syntax.comment").expect("valid tag"),
            Style::new().fg(Color::ansi(245)),
        );
        highlights.insert(
            Tag::parse("syntax.function").expect("valid tag"),
            Style::new().fg(Color::ansi(196)).bold(),
        );
        let _theme_guard = globals::set_test_active_theme(Theme::new(
            "test",
            ThemeKind::Ansi256,
            Style::new(),
            HighlightStyles::new(highlights),
        ));
        let _guard = globals::set_test_config(Config::default());
        let item = CompletionCandidate {
            label: "String::new".to_string(),
            replacement: "String::new()".to_string(),
            range: TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 11),
            },
            symbol: None,
            kind: Some(CompletionItemKind::FUNCTION),
            insert_format: None,
            detail: None,
            label_detail: None,
            label_description: None,
            additional_text_edits: Vec::new(),
            lsp_completion_item: None,
            deprecated: true,
            preselect: false,
        };

        let base_style = Style::new().bg(Color::ansi(17)).fg(Color::ansi(250));
        let segments = completion_row_segments(&item, base_style, "> ".to_string(), 40);
        let muted_row_style = base_style
            .accent(Style::new().fg(Color::ansi(245)))
            .faint()
            .italic()
            .strikethrough();

        assert_eq!(segments[0].style, muted_row_style);
        assert_eq!(segments[2].style, muted_row_style);
        assert_eq!(
            segments[1].style,
            muted_row_style.accent(Style::new().fg(Color::ansi(196)).bold())
        );
    }

    #[test]
    fn completion_label_and_detail_columns_use_distinct_styles() {
        let _theme_guard = globals::set_test_active_theme(Theme::new(
            "test",
            ThemeKind::Ansi256,
            Style::new(),
            HighlightStyles::default(),
        ));
        let _guard = globals::set_test_config(Config::default());
        let item = CompletionCandidate {
            label: "String::new".to_string(),
            replacement: "String::new()".to_string(),
            range: TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 11),
            },
            symbol: None,
            kind: Some(CompletionItemKind::FUNCTION),
            insert_format: None,
            detail: Some("fn()".to_string()),
            label_detail: Some("module".to_string()),
            label_description: Some("desc".to_string()),
            additional_text_edits: Vec::new(),
            lsp_completion_item: None,
            deprecated: false,
            preselect: false,
        };

        let base_style = Style::new().bg(Color::ansi(17));
        let segments = completion_row_segments(&item, base_style, "> ".to_string(), 80);

        assert_eq!(segments[3].style, base_style);
        assert_eq!(segments[5].style, base_style.faint().italic());
        assert_eq!(segments[7].style, base_style.faint());
    }
}
