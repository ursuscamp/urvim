//! Theme validation and resolution.

use std::collections::BTreeMap;

use urvim_terminal::{Color, Rgb, Style};

use super::Tag;
use super::error::ThemeLoadError;
use super::model::{HighlightStyles, StyleOverlay, Theme, ThemeKind};
use super::parser::parse_theme;
use super::schema::{RawColorValue, RawStyle, RawTheme};

/// Parses and resolves a TOML theme document in one step.
pub fn resolve_theme_from_str(source: &str, input: &str) -> Result<Theme, ThemeLoadError> {
    let raw = parse_theme(source, input)?;
    resolve_theme(raw)
}

/// Resolves a raw theme into a runtime theme.
pub fn resolve_theme(raw: RawTheme) -> Result<Theme, ThemeLoadError> {
    let theme_name = raw.name.trim();
    if theme_name.is_empty() {
        return Err(ThemeLoadError::InvalidThemeName(raw.name));
    }

    if raw.palette.is_empty() {
        return Err(ThemeLoadError::MissingSection {
            theme: theme_name.to_string(),
            section: "palette",
        });
    }

    let (palette, kind) = resolve_palette(theme_name, &raw.palette)?;
    let default_style = resolve_style(
        theme_name,
        "default",
        "default",
        Style::new(),
        &raw.default,
        &palette,
    )?;
    let highlights = resolve_highlight_styles(theme_name, &raw.highlights, &palette)?;

    Ok(Theme::new(theme_name, kind, default_style, highlights))
}

fn resolve_palette(
    theme_name: &str,
    raw_palette: &BTreeMap<String, RawColorValue>,
) -> Result<(BTreeMap<String, Color>, ThemeKind), ThemeLoadError> {
    let mut palette = BTreeMap::new();
    let mut kind = ThemeKind::Ansi256;

    for (name, value) in raw_palette {
        let color = match value {
            RawColorValue::Ansi(ansi) => Color::ansi(*ansi),
            RawColorValue::Rgb(value) => {
                kind = ThemeKind::TrueColor;
                parse_rgb(theme_name, name, value)?
            }
        };

        palette.insert(name.clone(), color);
    }

    Ok((palette, kind))
}

fn parse_rgb(theme_name: &str, key: &str, value: &str) -> Result<Color, ThemeLoadError> {
    Rgb::parse_hex(value)
        .map(Color::Rgb)
        .map_err(|_| ThemeLoadError::InvalidPaletteValue {
            theme: theme_name.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        })
}

fn resolve_highlight_styles(
    theme_name: &str,
    raw: &std::collections::BTreeMap<String, RawStyle>,
    palette: &BTreeMap<String, Color>,
) -> Result<HighlightStyles, ThemeLoadError> {
    let mut styles = HighlightStyles::default();
    for (highlight, raw_style) in raw {
        let tag = Tag::parse(highlight).map_err(|_| ThemeLoadError::InvalidTag {
            theme: theme_name.to_string(),
            section: "highlights",
            tag: highlight.to_string(),
        })?;
        let resolved = resolve_style(
            theme_name,
            "highlights",
            tag.as_str(),
            Style::default(),
            raw_style,
            palette,
        )?;
        styles.insert(tag, resolved);
    }

    Ok(styles)
}

fn resolve_style(
    theme_name: &str,
    section: &'static str,
    key: &str,
    base: Style,
    raw: &RawStyle,
    palette: &BTreeMap<String, Color>,
) -> Result<Style, ThemeLoadError> {
    let overlay_style = resolve_style_overlay(theme_name, section, key, raw, palette)?;
    let base = if raw.overlay { Style::default() } else { base };
    Ok(overlay_style.apply_to(base))
}

fn resolve_style_overlay(
    theme_name: &str,
    section: &'static str,
    key: &str,
    raw: &RawStyle,
    palette: &BTreeMap<String, Color>,
) -> Result<StyleOverlay, ThemeLoadError> {
    Ok(StyleOverlay {
        fg: resolve_palette_reference(theme_name, section, key, "fg", raw.fg.as_ref(), palette)?,
        bg: resolve_palette_reference(theme_name, section, key, "bg", raw.bg.as_ref(), palette)?,
        underline_color: resolve_palette_reference(
            theme_name,
            section,
            key,
            "underline_color",
            raw.underline_color.as_ref(),
            palette,
        )?,
        bold: raw.bold,
        italic: raw.italic,
        underline: raw.underline,
        double_underline: raw.double_underline,
        dim: raw.dim,
        reverse: raw.reverse,
        blink: raw.blink,
        strikethrough: raw.strikethrough,
        overline: raw.overline,
    })
}

fn resolve_palette_reference(
    theme_name: &str,
    section: &'static str,
    key: &str,
    field: &'static str,
    reference: Option<&String>,
    palette: &BTreeMap<String, Color>,
) -> Result<Option<Color>, ThemeLoadError> {
    match reference {
        Some(reference) => palette.get(reference).copied().map(Some).ok_or_else(|| {
            ThemeLoadError::UnknownPaletteReference {
                theme: theme_name.to_string(),
                section,
                key: format!("{key}.{field}"),
                reference: reference.to_string(),
            }
        }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tag, ThemeRegistry};
    use urvim_terminal::Rgb;

    /// Local stand-in for the notification severity level used by theme tests.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum NotificationLevel {
        Info,
        Warn,
        Error,
    }

    fn tag(value: &str) -> Tag {
        Tag::parse(value).expect("valid tag")
    }

    fn marker_style(theme: &str, marker: &str) -> Style {
        let fg = match theme {
            "Friday Night" => Color::ansi(16),
            "Saturday Morning" => Color::ansi(231),
            "Rose Pine" => Color::Rgb(Rgb::new(25, 23, 36)),
            "Dracula" => Color::Rgb(Rgb::new(40, 42, 54)),
            "Tokyo Night" => Color::Rgb(Rgb::new(26, 27, 38)),
            "Catppuccin" => Color::Rgb(Rgb::new(30, 30, 46)),
            "Nord" => Color::Rgb(Rgb::new(46, 52, 64)),
            "OneDark" => Color::Rgb(Rgb::new(24, 26, 31)),
            "Gruvbox" => Color::Rgb(Rgb::new(40, 40, 40)),
            "Gruvbox Light" => Color::Rgb(Rgb::new(251, 241, 199)),
            other => panic!("unexpected theme {other}"),
        };
        let bg = match marker {
            "todo" => match theme {
                "Friday Night" => Color::ansi(75),
                "Saturday Morning" => Color::ansi(24),
                "Rose Pine" => Color::Rgb(Rgb::new(196, 167, 231)),
                "Dracula" => Color::Rgb(Rgb::new(189, 147, 249)),
                "Tokyo Night" => Color::Rgb(Rgb::new(187, 154, 247)),
                "Catppuccin" => Color::Rgb(Rgb::new(203, 166, 247)),
                "Nord" => Color::Rgb(Rgb::new(235, 203, 139)),
                "OneDark" => Color::Rgb(Rgb::new(198, 120, 221)),
                "Gruvbox" => Color::Rgb(Rgb::new(250, 189, 47)),
                "Gruvbox Light" => Color::Rgb(Rgb::new(181, 118, 20)),
                other => panic!("unexpected theme {other}"),
            },
            "fixme" => match theme {
                "Friday Night" => Color::ansi(203),
                "Saturday Morning" => Color::ansi(160),
                "Rose Pine" => Color::Rgb(Rgb::new(235, 111, 146)),
                "Dracula" => Color::Rgb(Rgb::new(255, 85, 85)),
                "Tokyo Night" => Color::Rgb(Rgb::new(247, 118, 142)),
                "Catppuccin" => Color::Rgb(Rgb::new(243, 139, 168)),
                "Nord" => Color::Rgb(Rgb::new(191, 97, 106)),
                "OneDark" => Color::Rgb(Rgb::new(232, 102, 113)),
                "Gruvbox" => Color::Rgb(Rgb::new(251, 73, 52)),
                "Gruvbox Light" => Color::Rgb(Rgb::new(157, 0, 6)),
                other => panic!("unexpected theme {other}"),
            },
            "bug" => match theme {
                "Friday Night" => Color::ansi(214),
                "Saturday Morning" => Color::ansi(166),
                "Rose Pine" => Color::Rgb(Rgb::new(235, 188, 186)),
                "Dracula" => Color::Rgb(Rgb::new(255, 184, 108)),
                "Tokyo Night" => Color::Rgb(Rgb::new(255, 158, 100)),
                "Catppuccin" => Color::Rgb(Rgb::new(250, 179, 135)),
                "Nord" => Color::Rgb(Rgb::new(208, 135, 112)),
                "OneDark" => Color::Rgb(Rgb::new(209, 154, 102)),
                "Gruvbox" => Color::Rgb(Rgb::new(254, 128, 25)),
                "Gruvbox Light" => Color::Rgb(Rgb::new(175, 58, 3)),
                other => panic!("unexpected theme {other}"),
            },
            "note" => match theme {
                "Friday Night" => Color::ansi(80),
                "Saturday Morning" => Color::ansi(31),
                "Rose Pine" => Color::Rgb(Rgb::new(156, 207, 216)),
                "Dracula" => Color::Rgb(Rgb::new(139, 233, 253)),
                "Tokyo Night" => Color::Rgb(Rgb::new(125, 207, 255)),
                "Catppuccin" => Color::Rgb(Rgb::new(148, 226, 213)),
                "Nord" => Color::Rgb(Rgb::new(143, 188, 187)),
                "OneDark" => Color::Rgb(Rgb::new(86, 182, 194)),
                "Gruvbox" => Color::Rgb(Rgb::new(142, 192, 124)),
                "Gruvbox Light" => Color::Rgb(Rgb::new(66, 123, 88)),
                other => panic!("unexpected theme {other}"),
            },
            other => panic!("unexpected marker {other}"),
        };

        Style::new().fg(fg).bg(bg).bold()
    }

    fn selection_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().bg(Color::ansi(235)),
            "Saturday Morning" => Style::new().bg(Color::ansi(254)),
            "Rose Pine" => Style::new().bg(Color::Rgb(Rgb::new(38, 35, 58))),
            "Dracula" => Style::new().bg(Color::Rgb(Rgb::new(68, 71, 90))),
            "Tokyo Night" => Style::new().bg(Color::Rgb(Rgb::new(41, 46, 66))),
            "Catppuccin" => Style::new().bg(Color::Rgb(Rgb::new(69, 71, 90))),
            "Nord" => Style::new().bg(Color::Rgb(Rgb::new(67, 76, 94))),
            "OneDark" => Style::new().bg(Color::Rgb(Rgb::new(59, 63, 76))),
            "Gruvbox" => Style::new().bg(Color::Rgb(Rgb::new(102, 92, 84))),
            "Gruvbox Light" => Style::new().bg(Color::Rgb(Rgb::new(189, 174, 147))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn active_line_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().bg(Color::ansi(235)),
            "Saturday Morning" => Style::new().bg(Color::ansi(254)),
            "Rose Pine" => Style::new().bg(Color::Rgb(Rgb::new(33, 32, 46))),
            "Dracula" => Style::new().bg(Color::Rgb(Rgb::new(68, 71, 90))),
            "Tokyo Night" => Style::new().bg(Color::Rgb(Rgb::new(36, 40, 59))),
            "Catppuccin" => Style::new().bg(Color::Rgb(Rgb::new(49, 50, 68))),
            "Nord" => Style::new().bg(Color::Rgb(Rgb::new(59, 66, 82))),
            "OneDark" => Style::new().bg(Color::Rgb(Rgb::new(49, 53, 63))),
            "Gruvbox" => Style::new().bg(Color::Rgb(Rgb::new(60, 56, 54))),
            "Gruvbox Light" => Style::new().bg(Color::Rgb(Rgb::new(235, 219, 178))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn active_gutter_line_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(252)),
            "Saturday Morning" => Style::new().fg(Color::ansi(16)),
            "Rose Pine" => Style::new().fg(Color::Rgb(Rgb::new(224, 222, 244))),
            "Dracula" => Style::new().fg(Color::Rgb(Rgb::new(248, 248, 242))),
            "Tokyo Night" => Style::new().fg(Color::Rgb(Rgb::new(192, 202, 245))),
            "Catppuccin" => Style::new().fg(Color::Rgb(Rgb::new(205, 214, 244))),
            "Nord" => Style::new().fg(Color::Rgb(Rgb::new(216, 222, 233))),
            "OneDark" => Style::new().fg(Color::Rgb(Rgb::new(171, 178, 191))),
            "Gruvbox" => Style::new().fg(Color::Rgb(Rgb::new(250, 189, 47))),
            "Gruvbox Light" => Style::new().fg(Color::Rgb(Rgb::new(181, 118, 20))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn split_border_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(244)),
            "Saturday Morning" => Style::new().fg(Color::ansi(241)),
            "Rose Pine" => Style::new().fg(Color::Rgb(Rgb::new(110, 106, 134))),
            "Dracula" => Style::new().fg(Color::Rgb(Rgb::new(98, 114, 164))),
            "Tokyo Night" => Style::new().fg(Color::Rgb(Rgb::new(59, 66, 97))),
            "Catppuccin" => Style::new().fg(Color::Rgb(Rgb::new(108, 112, 134))),
            "Nord" => Style::new().fg(Color::Rgb(Rgb::new(76, 86, 106))),
            "OneDark" => Style::new().fg(Color::Rgb(Rgb::new(92, 99, 112))),
            "Gruvbox" => Style::new().fg(Color::Rgb(Rgb::new(102, 92, 84))),
            "Gruvbox Light" => Style::new().fg(Color::Rgb(Rgb::new(189, 174, 147))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn split_border_resize_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(75)).bold(),
            "Saturday Morning" => Style::new().fg(Color::ansi(24)).bold(),
            "Rose Pine" => Style::new().fg(Color::Rgb(Rgb::new(196, 167, 231))).bold(),
            "Dracula" => Style::new().fg(Color::Rgb(Rgb::new(189, 147, 249))).bold(),
            "Tokyo Night" => Style::new().fg(Color::Rgb(Rgb::new(122, 162, 247))).bold(),
            "Catppuccin" => Style::new().fg(Color::Rgb(Rgb::new(203, 166, 247))).bold(),
            "Nord" => Style::new().fg(Color::Rgb(Rgb::new(129, 161, 193))).bold(),
            "OneDark" => Style::new().fg(Color::Rgb(Rgb::new(97, 175, 239))).bold(),
            "Gruvbox" => Style::new().fg(Color::Rgb(Rgb::new(254, 128, 25))).bold(),
            "Gruvbox Light" => Style::new().fg(Color::Rgb(Rgb::new(175, 58, 3))).bold(),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn notification_style(theme: &str, level: NotificationLevel) -> Style {
        match (theme, level) {
            ("Friday Night", NotificationLevel::Info) => Style::new().fg(Color::ansi(110)),
            ("Friday Night", NotificationLevel::Warn) => Style::new().fg(Color::ansi(221)).bold(),
            ("Friday Night", NotificationLevel::Error) => Style::new().fg(Color::ansi(203)).bold(),
            ("Saturday Morning", NotificationLevel::Info) => Style::new().fg(Color::ansi(25)),
            ("Saturday Morning", NotificationLevel::Warn) => {
                Style::new().fg(Color::ansi(172)).bold()
            }
            ("Saturday Morning", NotificationLevel::Error) => {
                Style::new().fg(Color::ansi(160)).bold()
            }
            ("Rose Pine", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(156, 207, 216)))
            }
            ("Rose Pine", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(246, 193, 119))).bold()
            }
            ("Rose Pine", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(235, 111, 146))).bold()
            }
            ("Dracula", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(139, 233, 253)))
            }
            ("Dracula", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(241, 250, 140))).bold()
            }
            ("Dracula", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(255, 85, 85))).bold()
            }
            ("Tokyo Night", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(122, 162, 247)))
            }
            ("Tokyo Night", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(224, 175, 104))).bold()
            }
            ("Tokyo Night", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(247, 118, 142))).bold()
            }
            ("Catppuccin", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(137, 180, 250)))
            }
            ("Catppuccin", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(249, 226, 175))).bold()
            }
            ("Catppuccin", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(243, 139, 168))).bold()
            }
            ("Nord", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(163, 190, 140)))
            }
            ("Nord", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(235, 203, 139))).bold()
            }
            ("Nord", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(191, 97, 106))).bold()
            }
            ("OneDark", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(152, 195, 121)))
            }
            ("OneDark", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(229, 192, 123))).bold()
            }
            ("OneDark", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(232, 102, 113))).bold()
            }
            ("Gruvbox", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(142, 192, 124)))
            }
            ("Gruvbox", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(250, 189, 47))).bold()
            }
            ("Gruvbox", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(251, 73, 52))).bold()
            }
            ("Gruvbox Light", NotificationLevel::Info) => {
                Style::new().fg(Color::Rgb(Rgb::new(66, 123, 88)))
            }
            ("Gruvbox Light", NotificationLevel::Warn) => {
                Style::new().fg(Color::Rgb(Rgb::new(181, 118, 20))).bold()
            }
            ("Gruvbox Light", NotificationLevel::Error) => {
                Style::new().fg(Color::Rgb(Rgb::new(157, 0, 6))).bold()
            }
            (other, _) => panic!("unexpected theme {other}"),
        }
    }

    fn sample_theme() -> &'static str {
        r##"
name = "demo"

[palette]
base = 0
accent = "#112233"

[default]
fg = "base"
bg = "accent"
bold = true

 [highlights]
"ui.status_bar" = { fg = "accent" }
"ui.status_bar.modified_marker" = { fg = "base", bold = true }
"ui.window.active_line" = { bg = "base", overlay = true }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window.gutter.diff.added" = { fg = "base" }
"ui.window.gutter.diff.deleted" = { fg = "base" }
"ui.window.gutter.diff.modified" = { fg = "base" }
"ui.window" = { fg = "base" }
"ui.window.lines" = { fg = "base" }
"ui.window.lines.resize" = { fg = "accent", bold = true }
"ui.input.prompt" = { fg = "accent", bold = true }
"ui.input.prompt.exact" = { fg = "accent", bold = true }
"ui.input.prompt.fuzzy" = { fg = "accent", italic = true }
"ui.picker.accent" = { fg = "accent", bold = true }
"ui.input.prompt.separator" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "accent" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "accent" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"##
    }

    #[test]
    fn parse_and_resolve_sample_theme() {
        let theme = resolve_theme_from_str("sample", sample_theme()).expect("theme should resolve");
        assert_eq!(theme.name(), "demo");
        assert_eq!(theme.kind(), ThemeKind::TrueColor);
        assert_eq!(
            theme.highlight_style_for_name("ui.status_bar"),
            Style::new().fg(Color::Rgb(Rgb::new(17, 34, 51)))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.keyword")),
            Style::new().fg(Color::Rgb(Rgb::new(17, 34, 51)))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.active_line"),
            Style::new().bg(Color::ansi(0))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.lines.border"),
            Style::new().fg(Color::ansi(0))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.lines.resize"),
            Style::new().fg(Color::Rgb(Rgb::new(17, 34, 51))).bold()
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.gutter.diff.added"),
            Style::new().fg(Color::ansi(0))
        );
    }

    #[test]
    fn resolves_comment_marker_tags_through_parent_lookup() {
        let theme = sample_theme().replace(
            "\"syntax.comment\" = { fg = \"base\" }\n",
            "\"syntax.comment\" = { fg = \"base\" }\n\"syntax.comment.todo\" = { fg = \"accent\" }\n",
        );
        let theme = resolve_theme_from_str("sample", &theme).expect("theme should resolve");

        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.todo")),
            Style::new().fg(Color::Rgb(Rgb::new(17, 34, 51)))
        );
    }

    #[test]
    fn rejects_unknown_palette_reference() {
        let theme = r#"
name = "demo"

[palette]
base = 0

[default]
fg = "base"

[highlights]
"ui.status_bar" = { fg = "missing" }
"ui.status_bar.modified_marker" = { fg = "base" }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window.gutter.diff.added" = { fg = "base" }
"ui.window.gutter.diff.deleted" = { fg = "base" }
"ui.window.gutter.diff.modified" = { fg = "base" }
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "base" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"#;

        let err = resolve_theme_from_str("sample", theme).expect_err("resolution should fail");
        assert!(matches!(
            err,
            ThemeLoadError::UnknownPaletteReference { .. }
        ));
    }

    #[test]
    fn rejects_invalid_highlight_name() {
        let theme = r#"
name = "demo"

[palette]
base = 0

[default]
fg = "base"

[highlights]
"ui.status_bar" = { fg = "base" }
"ui.status_bar.modified_marker" = { fg = "base" }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window.gutter.diff.added" = { fg = "base" }
"ui.window.gutter.diff.deleted" = { fg = "base" }
"ui.window.gutter.diff.modified" = { fg = "base" }
"ui.window" = { fg = "base" }
"ui.Extra" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "base" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"#;

        let err = resolve_theme_from_str("sample", theme).expect_err("validation should fail");
        assert!(matches!(err, ThemeLoadError::InvalidTag { .. }));
    }

    #[test]
    fn rejects_invalid_highlight_name_in_theme_document() {
        let theme = r#"
name = "demo"

[palette]
base = 0

[default]
fg = "base"

[highlights]
"ui.status_bar" = { fg = "base" }
"ui.status_bar.modified_marker" = { fg = "base" }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "base" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"ui.Extra" = { fg = "base" }
"#;

        let err = resolve_theme_from_str("sample", theme).expect_err("validation should fail");
        assert!(matches!(err, ThemeLoadError::InvalidTag { .. }));
    }

    #[test]
    fn resolves_partial_styles_against_default_style() {
        let theme = r#"
name = "demo"

[palette]
base = 0
accent = 1

[default]
fg = "base"
bg = "accent"
bold = true

[highlights]
"ui.status_bar" = { fg = "accent" }
"ui.status_bar.modified_marker" = { fg = "base", bold = true }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "accent" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"#;

        let theme = resolve_theme_from_str("sample", theme).expect("theme should resolve");
        assert_eq!(
            theme.highlight_style_for_name("ui.status_bar"),
            Style::new().fg(Color::ansi(1))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.keyword")),
            Style::new().fg(Color::ansi(1))
        );
    }

    #[test]
    fn resolves_overlay_styles_against_blank_style() {
        let theme = r#"
name = "demo"

[palette]
base = 0
accent = 1

[default]
fg = "base"
bg = "accent"
bold = true

[highlights]
"ui.status_bar" = { fg = "accent" }
"ui.status_bar.modified_marker" = { fg = "base", bold = true }
"ui.window.active_line" = { bg = "base", overlay = true }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "accent" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"#;

        let theme = resolve_theme_from_str("sample", theme).expect("theme should resolve");
        assert_eq!(
            theme.highlight_style_for_name("ui.window.active_line"),
            Style::new().bg(Color::ansi(0))
        );
    }

    #[test]
    fn rejects_invalid_palette_values() {
        let theme = r##"
name = "demo"

[palette]
base = "#zzzzzz"

[default]
fg = "base"
bg = "base"

[highlights]
"ui.status_bar" = { fg = "base" }
"ui.status_bar.modified_marker" = { fg = "base" }
"ui.tab.active" = { fg = "base" }
"ui.tab.inactive" = { fg = "base" }
"ui.tab.scroll_indicator" = { fg = "base" }
"ui.window.gutter" = { fg = "base" }
"ui.window.gutter.active_line" = { fg = "accent" }
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
"syntax.namespace" = { fg = "base" }
"syntax.keyword" = { fg = "base" }
"syntax.number" = { fg = "base" }
"syntax.operator" = { fg = "base" }
"syntax.punctuation" = { fg = "base" }
"syntax.string" = { fg = "base" }
"syntax.type" = { fg = "base" }
"syntax.variable" = { fg = "base" }
"##;

        let err = resolve_theme_from_str("sample", theme).expect_err("invalid palette should fail");
        assert!(matches!(err, ThemeLoadError::InvalidPaletteValue { .. }));
    }

    #[test]
    fn builtin_themes_parse_and_classify() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");
        let friday_night = registry.default_theme();

        fn count_unique_styles(styles: &[Style]) -> usize {
            let mut unique = Vec::new();
            for style in styles {
                if !unique.iter().any(|existing: &Style| existing == style) {
                    unique.push(style.clone());
                }
            }
            unique.len()
        }

        assert_eq!(registry.default_theme().name(), "Friday Night");
        assert_eq!(
            registry.get("Friday Night").unwrap().kind(),
            ThemeKind::Ansi256
        );
        assert_eq!(
            registry.get("Saturday Morning").unwrap().kind(),
            ThemeKind::Ansi256
        );
        assert_eq!(
            registry.get("Rose Pine").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(
            registry.get("Dracula").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(
            registry.get("Tokyo Night").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(
            registry.get("Catppuccin").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(registry.get("Nord").unwrap().kind(), ThemeKind::TrueColor);
        assert_eq!(
            registry.get("OneDark").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(
            registry.get("Gruvbox").unwrap().kind(),
            ThemeKind::TrueColor
        );
        assert_eq!(
            registry.get("Gruvbox Light").unwrap().kind(),
            ThemeKind::TrueColor
        );
        for name in registry.names() {
            let theme = registry.get(name).unwrap();
            let semantic_styles = [
                theme.highlight_style_for_tag(&tag("syntax.constant")),
                theme.highlight_style_for_tag(&tag("syntax.function")),
                theme.highlight_style_for_tag(&tag("syntax.namespace")),
                theme.highlight_style_for_tag(&tag("syntax.keyword")),
                theme.highlight_style_for_tag(&tag("syntax.number")),
                theme.highlight_style_for_tag(&tag("syntax.operator")),
                theme.highlight_style_for_tag(&tag("syntax.string")),
                theme.highlight_style_for_tag(&tag("syntax.type")),
                theme.highlight_style_for_tag(&tag("syntax.variable")),
                theme.highlight_style_for_tag(&tag("syntax.variable.global")),
            ];
            assert_eq!(
                theme.highlight_style_for_name("ui.selection"),
                selection_style(name),
                "theme {name} should define a visible visual selection style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.window.active_line"),
                active_line_style(name),
                "theme {name} should define an active line style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.window.gutter.active_line"),
                active_gutter_line_style(name),
                "theme {name} should define an active gutter line style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.window.lines.border"),
                split_border_style(name),
                "theme {name} should define a normal split border style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.window.lines.resize"),
                split_border_resize_style(name),
                "theme {name} should define a resize split border style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.notification.info"),
                notification_style(name, NotificationLevel::Info),
                "theme {name} should define an info notification style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.notification.warn"),
                notification_style(name, NotificationLevel::Warn),
                "theme {name} should define a warn notification style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.notification.error"),
                notification_style(name, NotificationLevel::Error),
                "theme {name} should define an error notification style"
            );
            let unique_style_count = count_unique_styles(&semantic_styles);
            assert!(
                unique_style_count >= 5,
                "theme {name} should give core code syntax tags a broad style spread"
            );
        }
        assert_eq!(
            friday_night
                .highlight_style_for_tag(&Tag::parse("syntax.string.interpolation").unwrap()),
            Style::new().fg(Color::ansi(75))
        );
        assert_eq!(
            friday_night.highlight_style_for_tag(&tag("syntax.variable.global")),
            Style::new().fg(Color::ansi(80))
        );
        assert_eq!(
            registry
                .get("Saturday Morning")
                .unwrap()
                .highlight_style_for_tag(&tag("syntax.variable.global")),
            Style::new().fg(Color::ansi(31))
        );
    }

    #[test]
    fn builtin_themes_expose_marker_specific_comment_styles() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");
        for name in [
            "Friday Night",
            "Saturday Morning",
            "Rose Pine",
            "Dracula",
            "Tokyo Night",
            "Catppuccin",
            "Nord",
            "OneDark",
            "Gruvbox",
            "Gruvbox Light",
        ] {
            let theme = registry.get(name).expect("theme should exist");
            let comment_style = theme.highlight_style_for_tag(&tag("syntax.comment"));
            for marker in ["todo", "fixme", "bug", "note"] {
                let style =
                    theme.highlight_style_for_tag(&tag(&format!("syntax.comment.{marker}")));
                assert_ne!(
                    style, comment_style,
                    "{name} should expose a distinct {marker} style"
                );
                assert_eq!(
                    style,
                    marker_style(name, marker),
                    "{name} should use the expected {marker} foreground/background pairing"
                );
            }
        }
    }
}
