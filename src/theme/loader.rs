//! Theme validation and resolution.

use std::collections::BTreeMap;

use crate::terminal::{Color, Rgb, Style};

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
    let highlights = resolve_highlight_styles(theme_name, &raw.highlights, default_style, &palette)?;

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
    default_style: Style,
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
            default_style,
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
    use crate::terminal::Rgb;
    use crate::theme::{Tag, ThemeRegistry};

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
            other => panic!("unexpected theme {other}"),
        };
        let bg = match marker {
            "todo" => match theme {
                "Friday Night" => Color::ansi(75),
                "Saturday Morning" => Color::ansi(24),
                "Rose Pine" => Color::Rgb(Rgb::new(196, 167, 231)),
                "Dracula" => Color::Rgb(Rgb::new(189, 147, 249)),
                "Tokyo Night" => Color::Rgb(Rgb::new(122, 162, 247)),
                "Catppuccin" => Color::Rgb(Rgb::new(203, 166, 247)),
                other => panic!("unexpected theme {other}"),
            },
            "fixme" => match theme {
                "Friday Night" => Color::ansi(203),
                "Saturday Morning" => Color::ansi(160),
                "Rose Pine" => Color::Rgb(Rgb::new(235, 111, 146)),
                "Dracula" => Color::Rgb(Rgb::new(255, 85, 85)),
                "Tokyo Night" => Color::Rgb(Rgb::new(247, 118, 142)),
                "Catppuccin" => Color::Rgb(Rgb::new(243, 139, 168)),
                other => panic!("unexpected theme {other}"),
            },
            "bug" => match theme {
                "Friday Night" => Color::ansi(214),
                "Saturday Morning" => Color::ansi(166),
                "Rose Pine" => Color::Rgb(Rgb::new(234, 154, 151)),
                "Dracula" => Color::Rgb(Rgb::new(255, 184, 108)),
                "Tokyo Night" => Color::Rgb(Rgb::new(255, 158, 100)),
                "Catppuccin" => Color::Rgb(Rgb::new(250, 179, 135)),
                other => panic!("unexpected theme {other}"),
            },
            "note" => match theme {
                "Friday Night" => Color::ansi(80),
                "Saturday Morning" => Color::ansi(31),
                "Rose Pine" => Color::Rgb(Rgb::new(156, 207, 216)),
                "Dracula" => Color::Rgb(Rgb::new(139, 233, 253)),
                "Tokyo Night" => Color::Rgb(Rgb::new(125, 207, 255)),
                "Catppuccin" => Color::Rgb(Rgb::new(148, 226, 213)),
                other => panic!("unexpected theme {other}"),
            },
            other => panic!("unexpected marker {other}"),
        };

        Style::new().fg(fg).bg(bg).bold()
    }

    fn selection_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(16)).bg(Color::ansi(252)),
            "Saturday Morning" => Style::new().fg(Color::ansi(231)).bg(Color::ansi(235)),
            "Rose Pine" => Style::new()
                .fg(Color::Rgb(Rgb::new(25, 23, 36)))
                .bg(Color::Rgb(Rgb::new(224, 222, 244))),
            "Dracula" => Style::new()
                .fg(Color::Rgb(Rgb::new(40, 42, 54)))
                .bg(Color::Rgb(Rgb::new(248, 248, 242))),
            "Tokyo Night" => Style::new()
                .fg(Color::Rgb(Rgb::new(26, 27, 38)))
                .bg(Color::Rgb(Rgb::new(192, 202, 245))),
            "Catppuccin" => Style::new()
                .fg(Color::Rgb(Rgb::new(30, 30, 46)))
                .bg(Color::Rgb(Rgb::new(205, 214, 244))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn active_line_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().bg(Color::ansi(235)),
            "Saturday Morning" => Style::new().bg(Color::ansi(254)),
            "Rose Pine" => Style::new().bg(Color::Rgb(Rgb::new(31, 29, 46))),
            "Dracula" => Style::new().bg(Color::Rgb(Rgb::new(68, 71, 90))),
            "Tokyo Night" => Style::new().bg(Color::Rgb(Rgb::new(36, 40, 59))),
            "Catppuccin" => Style::new().bg(Color::Rgb(Rgb::new(49, 50, 68))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn split_border_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(244)).bg(Color::ansi(16)),
            "Saturday Morning" => Style::new().fg(Color::ansi(241)).bg(Color::ansi(231)),
            "Rose Pine" => Style::new()
                .fg(Color::Rgb(Rgb::new(110, 106, 134)))
                .bg(Color::Rgb(Rgb::new(25, 23, 36))),
            "Dracula" => Style::new()
                .fg(Color::Rgb(Rgb::new(98, 114, 164)))
                .bg(Color::Rgb(Rgb::new(40, 42, 54))),
            "Tokyo Night" => Style::new()
                .fg(Color::Rgb(Rgb::new(86, 95, 137)))
                .bg(Color::Rgb(Rgb::new(26, 27, 38))),
            "Catppuccin" => Style::new()
                .fg(Color::Rgb(Rgb::new(108, 112, 134)))
                .bg(Color::Rgb(Rgb::new(30, 30, 46))),
            other => panic!("unexpected theme {other}"),
        }
    }

    fn split_border_resize_style(theme: &str) -> Style {
        match theme {
            "Friday Night" => Style::new().fg(Color::ansi(75)).bg(Color::ansi(16)).bold(),
            "Saturday Morning" => Style::new().fg(Color::ansi(24)).bg(Color::ansi(231)).bold(),
            "Rose Pine" => Style::new()
                .fg(Color::Rgb(Rgb::new(196, 167, 231)))
                .bg(Color::Rgb(Rgb::new(25, 23, 36)))
                .bold(),
            "Dracula" => Style::new()
                .fg(Color::Rgb(Rgb::new(189, 147, 249)))
                .bg(Color::Rgb(Rgb::new(40, 42, 54)))
                .bold(),
            "Tokyo Night" => Style::new()
                .fg(Color::Rgb(Rgb::new(122, 162, 247)))
                .bg(Color::Rgb(Rgb::new(26, 27, 38)))
                .bold(),
            "Catppuccin" => Style::new()
                .fg(Color::Rgb(Rgb::new(203, 166, 247)))
                .bg(Color::Rgb(Rgb::new(30, 30, 46)))
                .bold(),
            other => panic!("unexpected theme {other}"),
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
"ui.window" = { fg = "base" }
"ui.window.split_border" = { fg = "base" }
"ui.window.split_border.resize" = { fg = "accent", bold = true }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.keyword")),
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.active_line"),
            Style::new().bg(Color::ansi(0))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.split_border"),
            Style::new()
                .fg(Color::ansi(0))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.split_border.resize"),
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
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
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
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
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
"ui.window" = { fg = "base" }
"ui.Extra" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(1)).bold()
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.keyword")),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(1)).bold()
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
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
"ui.window" = { fg = "base" }
"syntax.comment" = { fg = "base" }
"syntax.constant" = { fg = "base" }
"syntax.function" = { fg = "base" }
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
        for name in registry.names() {
            let theme = registry.get(name).unwrap();
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
                theme.highlight_style_for_name("ui.window.split_border"),
                split_border_style(name),
                "theme {name} should define a normal split border style"
            );
            assert_eq!(
                theme.highlight_style_for_name("ui.window.split_border.resize"),
                split_border_resize_style(name),
                "theme {name} should define a resize split border style"
            );
        }
        assert_eq!(
            friday_night.highlight_style_for_tag(
                &Tag::parse("syntax.string.interpolation").unwrap()
            ),
            Style::new().fg(Color::ansi(75)).bg(Color::ansi(16))
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
        ] {
            let theme = registry.get(name).expect("theme should exist");
            let comment_style = theme.highlight_style_for_tag(&tag("syntax.comment"));
            for marker in ["todo", "fixme", "bug", "note"] {
                let style = theme.highlight_style_for_tag(&tag(&format!("syntax.comment.{marker}")));
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
