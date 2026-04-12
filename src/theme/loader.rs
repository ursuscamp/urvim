//! Theme validation and resolution.

use std::collections::BTreeMap;

use crate::terminal::{Color, Rgb, Style};

use super::Tag;
use super::error::ThemeLoadError;
use super::model::{StyleOverride, SyntaxTagStyles, Theme, ThemeKind, UiStyles};
use super::parser::parse_theme;
use super::schema::{RawColorValue, RawStyle, RawTheme, RawUiStyles};

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
    let ui_styles = resolve_ui_styles(theme_name, &raw.ui, default_style, &palette)?;
    let syntax_styles = resolve_syntax_styles(theme_name, &raw.syntax, default_style, &palette)?;

    Ok(Theme::new(
        theme_name,
        kind,
        default_style,
        ui_styles,
        syntax_styles,
    ))
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

fn resolve_ui_styles(
    theme_name: &str,
    raw: &RawUiStyles,
    default_style: Style,
    palette: &BTreeMap<String, Color>,
) -> Result<UiStyles, ThemeLoadError> {
    Ok(UiStyles::new(
        resolve_style(
            theme_name,
            "ui",
            "status_bar",
            default_style,
            &raw.status_bar,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "modified_marker",
            default_style,
            &raw.modified_marker,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "tab_active",
            default_style,
            &raw.tab_active,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "tab_inactive",
            default_style,
            &raw.tab_inactive,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "tab_scroll_indicator",
            default_style,
            &raw.tab_scroll_indicator,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "gutter",
            default_style,
            &raw.gutter,
            palette,
        )?,
        resolve_style(
            theme_name,
            "ui",
            "window",
            default_style,
            &raw.window,
            palette,
        )?,
    ))
}

fn resolve_syntax_styles(
    theme_name: &str,
    raw: &std::collections::BTreeMap<String, RawStyle>,
    default_style: Style,
    palette: &BTreeMap<String, Color>,
) -> Result<SyntaxTagStyles, ThemeLoadError> {
    let mut styles = SyntaxTagStyles::default();
    for (tag, raw_style) in raw {
        let tag = Tag::parse(tag).map_err(|_| ThemeLoadError::InvalidTag {
            theme: theme_name.to_string(),
            section: "syntax",
            tag: tag.to_string(),
        })?;
        let resolved = resolve_style(
            theme_name,
            "syntax",
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
    let override_style = resolve_style_override(theme_name, section, key, raw, palette)?;
    Ok(override_style.apply_to(base))
}

fn resolve_style_override(
    theme_name: &str,
    section: &'static str,
    key: &str,
    raw: &RawStyle,
    palette: &BTreeMap<String, Color>,
) -> Result<StyleOverride, ThemeLoadError> {
    Ok(StyleOverride {
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
    use crate::theme::{Tag, ThemeRegistry};
    use crate::terminal::Rgb;

    fn tag(value: &str) -> Tag {
        Tag::parse(value).expect("valid tag")
    }

    fn marker_style(theme: &str, marker: &str) -> Style {
        let fg = match theme {
            "Friday Night" => Color::ansi(252),
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

[ui]
status_bar = { fg = "accent" }
modified_marker = { fg = "base", bold = true }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "accent" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "accent" }
type = { fg = "base" }
variable = { fg = "base" }
"##
    }

    #[test]
    fn parse_and_resolve_sample_theme() {
        let theme = resolve_theme_from_str("sample", sample_theme()).expect("theme should resolve");
        assert_eq!(theme.name(), "demo");
        assert_eq!(theme.kind(), ThemeKind::TrueColor);
        assert_eq!(
            theme.ui.status_bar,
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
        );
        assert_eq!(
            theme.syntax_style_for_tag(&tag("keyword")),
            Style::new()
                .fg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bg(Color::Rgb(Rgb::new(17, 34, 51)))
                .bold()
        );
    }

    #[test]
    fn resolves_comment_marker_tags_through_parent_lookup() {
        let theme = sample_theme().replace(
            "comment = { fg = \"base\" }\n",
            "comment = { fg = \"base\" }\n\"comment.todo\" = { fg = \"accent\" }\n",
        );
        let theme = resolve_theme_from_str("sample", &theme).expect("theme should resolve");

        assert_eq!(
            theme.syntax_style_for_tag(&tag("comment.todo")),
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

[ui]
status_bar = { fg = "missing" }
modified_marker = { fg = "base" }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "base" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "base" }
type = { fg = "base" }
variable = { fg = "base" }
"#;

        let err = resolve_theme_from_str("sample", theme).expect_err("resolution should fail");
        assert!(matches!(
            err,
            ThemeLoadError::UnknownPaletteReference { .. }
        ));
    }

    #[test]
    fn rejects_unknown_ui_key() {
        let theme = r#"
name = "demo"

[palette]
base = 0

[default]
fg = "base"

[ui]
status_bar = { fg = "base" }
modified_marker = { fg = "base" }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }
extra = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "base" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "base" }
type = { fg = "base" }
variable = { fg = "base" }
"#;

        let err = resolve_theme_from_str("sample", theme).expect_err("validation should fail");
        match err {
            ThemeLoadError::Parse { message, .. } => {
                assert!(message.contains("extra"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_syntax_tag() {
        let theme = r#"
name = "demo"

[palette]
base = 0

[default]
fg = "base"

[ui]
status_bar = { fg = "base" }
modified_marker = { fg = "base" }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "base" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "base" }
type = { fg = "base" }
variable = { fg = "base" }
Extra = { fg = "base" }
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

[ui]
status_bar = { fg = "accent" }
modified_marker = { fg = "base", bold = true }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "accent" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "base" }
type = { fg = "base" }
variable = { fg = "base" }
"#;

        let theme = resolve_theme_from_str("sample", theme).expect("theme should resolve");
        assert_eq!(
            theme.ui.status_bar,
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(1)).bold()
        );
        assert_eq!(
            theme.syntax_style_for_tag(&tag("keyword")),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(1)).bold()
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

[ui]
status_bar = { fg = "base" }
modified_marker = { fg = "base" }
tab_active = { fg = "base" }
tab_inactive = { fg = "base" }
tab_scroll_indicator = { fg = "base" }
gutter = { fg = "base" }
window = { fg = "base" }

[syntax]
comment = { fg = "base" }
constant = { fg = "base" }
function = { fg = "base" }
keyword = { fg = "base" }
number = { fg = "base" }
operator = { fg = "base" }
punctuation = { fg = "base" }
string = { fg = "base" }
type = { fg = "base" }
variable = { fg = "base" }
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
        assert_eq!(
            friday_night.syntax_style_for_tag(&Tag::parse("string.interpolation").unwrap()),
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
            let comment_style = theme.syntax_style_for_tag(&tag("comment"));
            for marker in ["todo", "fixme", "bug", "note"] {
                let style = theme.syntax_style_for_tag(&tag(&format!("comment.{marker}")));
                assert_ne!(
                    style,
                    comment_style,
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
