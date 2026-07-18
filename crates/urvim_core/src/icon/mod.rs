//! Centralized icon and glyph lookup helpers.

use crate::editor_tab::FoldGutterGlyph;
use crate::globals;
use lsp_types::{CompletionItemKind, DiagnosticSeverity, SymbolKind};
use smol_str::SmolStr;
use std::path::Path;
use urvim_syntax::{SyntaxMetadata, builtin_syntax_registry};
use urvim_terminal::Style;

/// A filetype icon paired with the style used to render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiletypeIcon {
    /// Icon text.
    pub glyph: SmolStr,
    /// Icon style.
    pub style: Style,
}

impl FiletypeIcon {
    /// Resolves a filetype icon for a path using the active icon configuration.
    pub fn from_path(path: &Path) -> Option<Self> {
        let nerdfont_enabled = nerdfont_enabled();
        if !nerdfont_enabled {
            return None;
        }

        let registry = builtin_syntax_registry().ok()?;
        let syntax_name = registry.resolve_for_input(Some(path), None)?;
        let metadata = registry.metadata(syntax_name.as_str())?;
        Self::from_metadata(Some(&metadata), nerdfont_enabled)
    }

    /// Resolves a filetype icon for syntax metadata, if icon rendering is enabled.
    pub fn from_metadata(
        metadata: Option<&SyntaxMetadata>,
        nerdfont_enabled: bool,
    ) -> Option<Self> {
        if !nerdfont_enabled {
            return None;
        }

        let metadata = metadata?;
        let glyph = metadata.glyph.clone()?;
        let style = metadata
            .glyph_color
            .map(|color| Style::default().fg(color))
            .unwrap_or_default();

        Some(Self { glyph, style })
    }
}

/// Glyph set used to draw bordered overlays and separators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderGlyphs {
    /// Top-left corner glyph.
    pub top_left: &'static str,
    /// Top-right corner glyph.
    pub top_right: &'static str,
    /// Bottom-left corner glyph.
    pub bottom_left: &'static str,
    /// Bottom-right corner glyph.
    pub bottom_right: &'static str,
    /// Horizontal line glyph.
    pub horizontal: &'static str,
    /// Vertical line glyph.
    pub vertical: &'static str,
    /// Left separator junction glyph.
    pub separator_left: &'static str,
    /// Right separator junction glyph.
    pub separator_right: &'static str,
}

impl BorderGlyphs {
    /// Returns border glyphs enabled by the active configuration.
    pub fn active() -> Self {
        Self::for_unicode_borders(unicode_borders_enabled())
    }

    /// Returns border glyphs for the requested border capability.
    pub fn for_unicode_borders(unicode_borders: bool) -> Self {
        if unicode_borders {
            return Self {
                top_left: "┌",
                top_right: "┐",
                bottom_left: "└",
                bottom_right: "┘",
                horizontal: "─",
                vertical: "│",
                separator_left: "├",
                separator_right: "┤",
            };
        }

        Self {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            horizontal: "-",
            vertical: "|",
            separator_left: "|",
            separator_right: "|",
        }
    }
}

/// Returns whether Nerd Font icons are enabled.
pub fn nerdfont_enabled() -> bool {
    globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false)
}

/// Returns whether Unicode borders are enabled.
pub fn unicode_borders_enabled() -> bool {
    globals::with_config(|config| config.unicode_borders_enabled()).unwrap_or(false)
}

/// Returns whether Unicode fold gutter glyph rendering is enabled.
pub fn unicode_folds_enabled() -> bool {
    globals::with_config(|config| config.unicode_folds_enabled()).unwrap_or(false)
}

/// Returns the fold gutter glyph for the active Unicode capability.
pub fn fold_gutter_glyph(glyph: FoldGutterGlyph, unicode_enabled: bool) -> &'static str {
    match (glyph, unicode_enabled) {
        (FoldGutterGlyph::Open, true) => "▼",
        (FoldGutterGlyph::Closed, true) => "▶",
        (FoldGutterGlyph::Open, false) => "v",
        (FoldGutterGlyph::Closed, false) => ">",
    }
}

/// Returns the picker or completion forward selection indicator.
pub fn selection_indicator() -> &'static str {
    if nerdfont_enabled() { "" } else { ">" }
}

/// Returns the picker backward selection indicator.
pub fn backward_selection_indicator() -> &'static str {
    if nerdfont_enabled() { "‹" } else { "<" }
}

/// Returns a selection prefix containing the active indicator and trailing space.
pub fn selection_prefix() -> String {
    format!("{} ", selection_indicator())
}

/// Returns the display marker for a diagnostic severity.
pub fn diagnostic_marker(severity: DiagnosticSeverity, nerdfont_enabled: bool) -> &'static str {
    if nerdfont_enabled {
        match severity {
            DiagnosticSeverity::ERROR => "",
            DiagnosticSeverity::WARNING => "",
            DiagnosticSeverity::INFORMATION => "",
            DiagnosticSeverity::HINT => "",
            _ => "",
        }
    } else {
        match severity {
            DiagnosticSeverity::ERROR => "E",
            DiagnosticSeverity::WARNING => "W",
            DiagnosticSeverity::INFORMATION => "I",
            DiagnosticSeverity::HINT => "H",
            _ => "I",
        }
    }
}

/// Returns a completion item kind badge for the active icon configuration.
pub fn completion_item_kind_badge(kind: CompletionItemKind) -> String {
    if nerdfont_enabled() {
        format!("{} ", completion_item_kind_icon(kind).unwrap_or(""))
    } else {
        completion_item_kind_abbreviation(kind).to_string()
    }
}

/// Returns a Nerd Font completion icon for a completion item kind.
pub fn completion_item_kind_icon(kind: CompletionItemKind) -> Option<&'static str> {
    Some(match kind {
        CompletionItemKind::TEXT => "",
        CompletionItemKind::METHOD => "",
        CompletionItemKind::FUNCTION => "󰊕",
        CompletionItemKind::CONSTRUCTOR => "",
        CompletionItemKind::FIELD => "",
        CompletionItemKind::VARIABLE => "",
        CompletionItemKind::CLASS => "",
        CompletionItemKind::INTERFACE => "",
        CompletionItemKind::MODULE => "",
        CompletionItemKind::PROPERTY => "",
        CompletionItemKind::UNIT => "",
        CompletionItemKind::VALUE => "",
        CompletionItemKind::ENUM => "",
        CompletionItemKind::KEYWORD => "",
        CompletionItemKind::SNIPPET => "",
        CompletionItemKind::COLOR => "",
        CompletionItemKind::FILE => "",
        CompletionItemKind::REFERENCE => "",
        CompletionItemKind::FOLDER => "",
        CompletionItemKind::ENUM_MEMBER => "",
        CompletionItemKind::CONSTANT => "",
        CompletionItemKind::STRUCT => "",
        CompletionItemKind::EVENT => "",
        CompletionItemKind::OPERATOR => "",
        CompletionItemKind::TYPE_PARAMETER => "",
        _ => return None,
    })
}

/// Returns an ASCII completion item kind abbreviation.
pub fn completion_item_kind_abbreviation(kind: CompletionItemKind) -> &'static str {
    match kind {
        CompletionItemKind::TEXT => "tx ",
        CompletionItemKind::METHOD => "fn ",
        CompletionItemKind::FUNCTION => "fn ",
        CompletionItemKind::CONSTRUCTOR => "ct ",
        CompletionItemKind::FIELD => "fd ",
        CompletionItemKind::VARIABLE => "vr ",
        CompletionItemKind::CLASS => "cl ",
        CompletionItemKind::INTERFACE => "if ",
        CompletionItemKind::MODULE => "md ",
        CompletionItemKind::PROPERTY => "pr ",
        CompletionItemKind::UNIT => "un ",
        CompletionItemKind::VALUE => "vl ",
        CompletionItemKind::ENUM => "en ",
        CompletionItemKind::KEYWORD => "kw ",
        CompletionItemKind::SNIPPET => "sn ",
        CompletionItemKind::COLOR => "co ",
        CompletionItemKind::FILE => "fi ",
        CompletionItemKind::REFERENCE => "rf ",
        CompletionItemKind::FOLDER => "fo ",
        CompletionItemKind::ENUM_MEMBER => "em ",
        CompletionItemKind::CONSTANT => "cn ",
        CompletionItemKind::STRUCT => "st ",
        CompletionItemKind::EVENT => "ev ",
        CompletionItemKind::OPERATOR => "op ",
        CompletionItemKind::TYPE_PARAMETER => "tp ",
        _ => "?? ",
    }
}

/// Returns a Nerd Font document symbol icon for the active icon configuration.
pub fn symbol_kind_icon(kind: SymbolKind) -> Option<&'static str> {
    if !nerdfont_enabled() {
        return None;
    }

    Some(match kind {
        SymbolKind::FILE => "",
        SymbolKind::MODULE | SymbolKind::NAMESPACE | SymbolKind::PACKAGE => "",
        SymbolKind::CLASS | SymbolKind::STRUCT => "",
        SymbolKind::INTERFACE => "",
        SymbolKind::ENUM => "",
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => "",
        SymbolKind::PROPERTY | SymbolKind::FIELD | SymbolKind::VARIABLE | SymbolKind::CONSTANT => {
            ""
        }
        SymbolKind::STRING
        | SymbolKind::NUMBER
        | SymbolKind::BOOLEAN
        | SymbolKind::ARRAY
        | SymbolKind::OBJECT
        | SymbolKind::KEY
        | SymbolKind::NULL
        | SymbolKind::ENUM_MEMBER => "",
        SymbolKind::EVENT | SymbolKind::OPERATOR => "",
        SymbolKind::TYPE_PARAMETER => "",
        _ => "",
    })
}

/// Returns the buffer-word completion icon for the active configuration.
pub fn buffer_word_completion_symbol() -> Option<String> {
    nerdfont_enabled().then(|| " ".to_string())
}

/// Returns the fallback path completion icon for the active configuration.
pub fn fallback_path_completion_symbol() -> Option<String> {
    nerdfont_enabled().then(|| " ".to_string())
}

/// Returns the indentation guide glyph for the requested capability.
pub fn indent_guide_glyph(unicode_indent: bool) -> &'static str {
    if unicode_indent { "│" } else { "|" }
}

/// Returns a split-border glyph for the requested connection shape.
pub fn split_border_glyph(
    unicode: bool,
    north: bool,
    south: bool,
    west: bool,
    east: bool,
    vertical: bool,
    horizontal: bool,
) -> &'static str {
    if unicode {
        let connected_vertical = north || south || vertical;
        let connected_horizontal = west || east || horizontal;

        if connected_vertical && connected_horizontal {
            if north && south && west && east {
                "┼"
            } else if north && south && west {
                "┤"
            } else if north && south && east {
                "├"
            } else if north && west && east {
                "┴"
            } else if south && west && east {
                "┬"
            } else if north && east {
                "└"
            } else if north && west {
                "┘"
            } else if south && east {
                "┌"
            } else if south && west {
                "┐"
            } else {
                "┼"
            }
        } else if connected_vertical || vertical {
            "│"
        } else if connected_horizontal || horizontal {
            "─"
        } else {
            " "
        }
    } else if (north || south || vertical) && (west || east || horizontal) {
        "+"
    } else if north || south || vertical {
        "|"
    } else if west || east || horizontal {
        "-"
    } else {
        " "
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdvancedGlyphCapability, Config};
    use std::collections::BTreeSet;

    #[test]
    fn selection_indicator_follows_nerdfont_capability() {
        let _guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        assert_eq!(selection_indicator(), "");
        assert_eq!(backward_selection_indicator(), "‹");
        assert_eq!(selection_prefix(), " ");
    }

    #[test]
    fn selection_indicator_uses_ascii_without_nerdfont() {
        let _guard = globals::set_test_config(Config::default());

        assert_eq!(selection_indicator(), ">");
        assert_eq!(backward_selection_indicator(), "<");
        assert_eq!(selection_prefix(), "> ");
    }

    #[test]
    fn diagnostic_marker_follows_nerdfont_capability() {
        assert_eq!(diagnostic_marker(DiagnosticSeverity::ERROR, true), "");
        assert_eq!(diagnostic_marker(DiagnosticSeverity::ERROR, false), "E");
    }

    #[test]
    fn completion_kind_badge_follows_nerdfont_capability() {
        let _guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        assert_eq!(
            completion_item_kind_badge(CompletionItemKind::FUNCTION),
            "󰊕 "
        );
    }

    #[test]
    fn completion_kind_badge_uses_ascii_without_nerdfont() {
        let _guard = globals::set_test_config(Config::default());

        assert_eq!(
            completion_item_kind_badge(CompletionItemKind::FUNCTION),
            "fn "
        );
    }

    #[test]
    fn symbol_kind_icon_requires_nerdfont() {
        let _guard = globals::set_test_config(Config::default());

        assert_eq!(symbol_kind_icon(SymbolKind::FUNCTION), None);
    }

    #[test]
    fn border_glyphs_follow_unicode_capability() {
        let ascii = BorderGlyphs::for_unicode_borders(false);
        let unicode = BorderGlyphs::for_unicode_borders(true);

        assert_eq!(ascii.horizontal, "-");
        assert_eq!(unicode.horizontal, "─");
        assert_eq!(unicode.separator_left, "├");
    }

    #[test]
    fn split_border_glyph_resolves_unicode_junctions() {
        assert_eq!(
            split_border_glyph(true, true, true, true, true, true, true),
            "┼"
        );
        assert_eq!(
            split_border_glyph(true, true, true, true, false, true, true),
            "┤"
        );
        assert_eq!(
            split_border_glyph(false, true, true, true, true, true, true),
            "+"
        );
    }
}
