use regex::Regex;
use smol_str::SmolStr;
use urvim_terminal::{Color, Rgb};

use super::error::SyntaxLoadError;
use super::normalize::normalize_label;

/// Selects the tokenizer used for a syntax definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTokenizer {
    /// Plain text tokenizer that emits no spans.
    Plaintext,
    /// Builtin scanner for Bash syntax.
    Bash,
    /// Builtin scanner for C syntax.
    C,
    /// Builtin scanner for C# syntax.
    Csharp,
    /// Builtin scanner for CMake syntax.
    Cmake,
    /// Builtin scanner for C++ syntax.
    Cpp,
    /// Builtin scanner for CSS syntax.
    Css,
    /// Builtin scanner for Dart syntax.
    Dart,
    /// Builtin scanner for Dockerfile syntax.
    Dockerfile,
    /// Builtin scanner for Elixir syntax.
    Elixir,
    /// Builtin scanner for Erlang syntax.
    Erlang,
    /// Builtin scanner for Fish syntax.
    Fish,
    /// Builtin scanner for F# syntax.
    Fsharp,
    /// Builtin scanner for Go syntax.
    Go,
    /// Builtin scanner for Haskell syntax.
    Haskell,
    /// Builtin scanner for HTML syntax.
    Html,
    /// Builtin scanner for Java syntax.
    Java,
    /// Builtin scanner for JavaScript syntax.
    Javascript,
    /// Builtin scanner for JSON syntax.
    Json,
    /// Builtin scanner for Julia syntax.
    Julia,
    /// Builtin scanner for Kotlin syntax.
    Kotlin,
    /// Builtin scanner for Justfile syntax.
    Justfile,
    /// Builtin scanner for Makefile syntax.
    Makefile,
    /// Builtin scanner for Markdown syntax.
    Markdown,
    /// Builtin scanner for Nim syntax.
    Nim,
    /// Builtin scanner for OCaml syntax.
    Ocaml,
    /// Builtin scanner for Perl syntax.
    Perl,
    /// Builtin scanner for PHP syntax.
    Php,
    /// Builtin scanner for Python syntax.
    Python,
    /// Builtin scanner for PowerShell syntax.
    Powershell,
    /// Builtin scanner for R syntax.
    R,
    /// Builtin scanner for Ruby syntax.
    Ruby,
    /// Builtin scanner for Rust syntax.
    Rust,
    /// Builtin scanner for Scala syntax.
    Scala,
    /// Builtin scanner for Shell syntax.
    Shell,
    /// Builtin scanner for Swift syntax.
    Swift,
    /// Builtin scanner for TOML syntax.
    Toml,
    /// Builtin scanner for TypeScript syntax.
    Typescript,
    /// Builtin scanner for YAML syntax.
    Yaml,
    /// Builtin scanner for Zsh syntax.
    Zsh,
    /// Builtin scanner for Zig syntax.
    Zig,
}

/// Static syntax metadata used to build compiled syntax definitions.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxMetadataSpec {
    /// The canonical syntax name.
    pub name: &'static str,
    /// The user-facing display label.
    pub display_name: &'static str,
    /// Alternate labels that resolve to the same syntax.
    pub alias: &'static [&'static str],
    /// The canonical line comment prefix used by comment toggle actions.
    pub comment_prefix: Option<&'static str>,
    /// The optional filetype glyph used in compact UI surfaces.
    pub glyph: Option<&'static str>,
    /// The optional default color for the filetype glyph.
    pub glyph_color: Option<&'static str>,
    /// Filename regex patterns used for syntax resolution.
    pub filename: &'static [&'static str],
    /// Shebang regex patterns used for syntax resolution.
    pub shebang: &'static [&'static str],
}

/// Static syntax definition data for a built-in tokenizer.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxSpec {
    /// Static metadata for the syntax.
    pub metadata: SyntaxMetadataSpec,
    /// Builtin scanner for the syntax.
    pub tokenizer: SyntaxTokenizer,
    /// Whether the syntax supports syntax-based folding.
    pub supports_folding: bool,
}

/// A compiled syntax definition.
#[derive(Debug, Clone)]
pub struct SyntaxDefinition {
    /// Syntax metadata used for resolution and display.
    pub metadata: SyntaxMetadata,
    /// Builtin scanner used for highlighting.
    pub tokenizer: SyntaxTokenizer,
    /// Whether the syntax supports syntax-based folding.
    pub supports_folding: bool,
}

impl SyntaxDefinition {
    /// Creates a compiled syntax definition from a static built-in spec.
    pub fn from_spec(spec: &SyntaxSpec) -> Result<Self, SyntaxLoadError> {
        Ok(Self {
            metadata: SyntaxMetadata::from_spec(&spec.metadata)?,
            tokenizer: spec.tokenizer,
            supports_folding: spec.supports_folding,
        })
    }

    /// Returns the canonical syntax name.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns the user-facing syntax label.
    pub fn display_name(&self) -> &str {
        &self.metadata.display_name
    }

    /// Returns the syntax glyph, if one is configured.
    pub fn glyph(&self) -> Option<&str> {
        self.metadata.glyph.as_deref()
    }

    /// Returns the syntax glyph color, if one is configured.
    pub fn glyph_color(&self) -> Option<Color> {
        self.metadata.glyph_color
    }
}

/// Compiled syntax metadata used for matching and display.
#[derive(Debug, Clone)]
pub struct SyntaxMetadata {
    /// The canonical syntax name.
    pub name: SmolStr,
    /// The user-facing display label.
    pub display_name: SmolStr,
    /// Alternate labels that resolve to the same syntax.
    pub alias: Vec<SmolStr>,
    /// The canonical line comment prefix used by comment toggle actions.
    pub comment_prefix: Option<SmolStr>,
    /// The optional filetype glyph used in compact UI surfaces.
    pub glyph: Option<SmolStr>,
    /// The optional default color for the filetype glyph.
    pub glyph_color: Option<Color>,
    /// Compiled filename regexes.
    pub filename: Vec<Regex>,
    /// Compiled shebang regexes.
    pub shebang: Vec<Regex>,
}

impl SyntaxMetadata {
    /// Creates compiled metadata from a static built-in spec.
    pub fn from_spec(spec: &SyntaxMetadataSpec) -> Result<Self, SyntaxLoadError> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(SyntaxLoadError::InvalidSyntaxName(spec.name.to_string()));
        }
        let name = SmolStr::new(name.to_ascii_lowercase());

        let display_name = SmolStr::new(spec.display_name.trim());
        if display_name.is_empty() {
            return Err(SyntaxLoadError::InvalidSyntaxName(
                spec.display_name.to_string(),
            ));
        }

        let mut aliases = Vec::with_capacity(spec.alias.len());
        for alias in spec.alias {
            let alias =
                normalize_label(alias).ok_or_else(|| SyntaxLoadError::InvalidSyntaxAlias {
                    syntax: name.to_string(),
                    alias: (*alias).to_string(),
                })?;
            if alias == name || aliases.iter().any(|existing| existing == &alias) {
                return Err(SyntaxLoadError::DuplicateSyntaxAlias {
                    syntax: name.to_string(),
                    alias: alias.to_string(),
                });
            }
            aliases.push(alias);
        }

        let comment_prefix = spec
            .comment_prefix
            .map(|prefix| {
                let prefix = prefix.trim();
                if prefix.is_empty() {
                    Err(SyntaxLoadError::InvalidCommentPrefix {
                        syntax: name.to_string(),
                        comment_prefix: prefix.to_string(),
                    })
                } else {
                    Ok(SmolStr::new(prefix))
                }
            })
            .transpose()?;

        let glyph = spec
            .glyph
            .map(|glyph| {
                let glyph = glyph.trim();
                if glyph.is_empty() {
                    Err(SyntaxLoadError::InvalidGlyph {
                        syntax: name.to_string(),
                        glyph: glyph.to_string(),
                    })
                } else {
                    Ok(SmolStr::new(glyph))
                }
            })
            .transpose()?;

        let glyph_color = spec
            .glyph_color
            .map(|color| parse_rgb_color(name.as_str(), color))
            .transpose()?;

        let filename = compile_patterns(name.as_str(), spec.filename)?;
        let shebang = compile_patterns(name.as_str(), spec.shebang)?;

        Ok(Self {
            name,
            display_name,
            alias: aliases,
            comment_prefix,
            glyph,
            glyph_color,
            filename,
            shebang,
        })
    }
}

fn compile_patterns(syntax: &str, patterns: &[&str]) -> Result<Vec<Regex>, SyntaxLoadError> {
    patterns
        .iter()
        .filter_map(|pattern| {
            let pattern = pattern.trim();
            (!pattern.is_empty()).then_some(pattern)
        })
        .map(|pattern| {
            Regex::new(pattern).map_err(|error| SyntaxLoadError::InvalidRegex {
                syntax: syntax.to_string(),
                pattern: pattern.to_string(),
                message: error.to_string(),
            })
        })
        .collect()
}

fn parse_rgb_color(syntax: &str, value: &str) -> Result<Color, SyntaxLoadError> {
    Rgb::parse_hex(value.trim())
        .map(Color::Rgb)
        .map_err(|_| SyntaxLoadError::InvalidGlyphColor {
            syntax: syntax.to_string(),
            color: value.to_string(),
        })
}
