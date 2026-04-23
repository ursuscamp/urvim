use crate::terminal::{Color, Rgb};
use crate::theme::Tag;
use regex::Regex;
use smol_str::SmolStr;

use super::definition::*;
use super::error::SyntaxLoadError;
use super::normalize::{normalize_context_marker, normalize_label};

pub(super) fn parse_syntax(
    source: &str,
    input: &str,
) -> Result<RawSyntaxDefinition, SyntaxLoadError> {
    toml::from_str::<RawSyntaxDefinition>(input).map_err(|error| SyntaxLoadError::Parse {
        source: source.to_string(),
        message: error.to_string(),
    })
}

pub(super) fn resolve_metadata(raw: &RawSyntaxMetadata) -> Result<SyntaxMetadata, SyntaxLoadError> {
    let name = raw.name.trim();
    if name.is_empty() {
        return Err(SyntaxLoadError::InvalidSyntaxName(raw.name.clone()));
    }
    let name = SmolStr::new(name.to_ascii_lowercase());

    let display_name = SmolStr::new(raw.display_name.trim());
    if display_name.is_empty() {
        return Err(SyntaxLoadError::InvalidSyntaxName(raw.display_name.clone()));
    }

    let mut aliases = Vec::with_capacity(raw.alias.len());
    for alias in &raw.alias {
        let alias = normalize_label(alias).ok_or_else(|| SyntaxLoadError::InvalidSyntaxAlias {
            syntax: name.to_string(),
            alias: alias.clone(),
        })?;
        if alias == name || aliases.iter().any(|existing| existing == &alias) {
            return Err(SyntaxLoadError::DuplicateSyntaxAlias {
                syntax: name.to_string(),
                alias: alias.to_string(),
            });
        }
        aliases.push(alias);
    }

    let comment_prefix = raw
        .comment_prefix
        .as_ref()
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

    let glyph = raw
        .glyph
        .as_ref()
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

    let glyph_color = raw
        .glyph_color
        .as_ref()
        .map(|color| resolve_glyph_color(name.as_str(), color))
        .transpose()?;

    let mut filename = Vec::with_capacity(raw.filename.len());
    for pattern in &raw.filename {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        let regex = Regex::new(pattern).map_err(|error| SyntaxLoadError::InvalidRegex {
            syntax: name.to_string(),
            pattern: pattern.to_string(),
            message: error.to_string(),
        })?;
        filename.push(regex);
    }

    let mut shebang = Vec::with_capacity(raw.shebang.len());
    for pattern in &raw.shebang {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        let regex = Regex::new(pattern).map_err(|error| SyntaxLoadError::InvalidRegex {
            syntax: name.to_string(),
            pattern: pattern.to_string(),
            message: error.to_string(),
        })?;
        shebang.push(regex);
    }

    Ok(SyntaxMetadata {
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

pub(super) fn resolve_syntax(
    raw: RawSyntaxDefinition,
    _source: &str,
) -> Result<SyntaxDefinition, SyntaxLoadError> {
    let metadata = resolve_metadata(&raw.metadata)?;
    let name = metadata.name.clone();
    let mut compiled_rules = Vec::with_capacity(raw.rules.len());
    for rule in raw.rules {
        compiled_rules.push(compile_rule(&name, rule)?);
    }

    Ok(SyntaxDefinition {
        metadata,
        rules: compiled_rules,
    })
}

fn compile_rule(syntax: &str, rule: RawRule) -> Result<SyntaxRule, SyntaxLoadError> {
    match rule {
        RawRule::Regex {
            pattern,
            lookahead,
            tag,
            context,
        } => {
            let tag = parse_tag(syntax, &tag)?;
            let regex = Regex::new(&pattern).map_err(|error| SyntaxLoadError::InvalidRegex {
                syntax: syntax.to_string(),
                pattern: pattern.clone(),
                message: error.to_string(),
            })?;
            let lookahead = lookahead
                .as_ref()
                .map(|pattern| {
                    Regex::new(pattern).map_err(|error| SyntaxLoadError::InvalidRegex {
                        syntax: syntax.to_string(),
                        pattern: pattern.clone(),
                        message: error.to_string(),
                    })
                })
                .transpose()?;
            Ok(SyntaxRule::Regex {
                regex,
                lookahead,
                tag,
                context: compile_context_control(syntax, context.as_ref())?,
            })
        }
        RawRule::Injection {
            selector,
            fallback,
            context,
        } => {
            let selector = compile_injection_selector(syntax, selector)?;
            let fallback = match fallback {
                RawInjectedSyntaxFallback::ParentStyle => InjectedSyntaxFallback::ParentStyle,
                RawInjectedSyntaxFallback::Unstyled => InjectedSyntaxFallback::Unstyled,
            };
            Ok(SyntaxRule::Injection {
                selector,
                fallback,
                context: compile_context_control(syntax, context.as_ref())?,
            })
        }
    }
}

fn compile_context_control(
    syntax: &str,
    raw: Option<&RawContextControl>,
) -> Result<Option<ContextControl>, SyntaxLoadError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let mut requires = Vec::with_capacity(raw.requires.len());
    for marker in &raw.requires {
        requires.push(normalize_context_marker(syntax, marker)?);
    }

    let mut push = Vec::with_capacity(raw.push.len());
    for marker in &raw.push {
        push.push(match marker {
            RawContextPush::Name(name) => ContextPush {
                name: normalize_context_marker(syntax, name)?,
                capture: None,
            },
            RawContextPush::Capture { name, capture } => ContextPush {
                name: normalize_context_marker(syntax, name)?,
                capture: Some(*capture),
            },
        });
    }

    let mut pop = Vec::with_capacity(raw.pop.len());
    for marker in &raw.pop {
        pop.push(normalize_context_marker(syntax, marker)?);
    }

    Ok(Some(ContextControl {
        requires,
        push,
        pop,
        payload_match: raw
            .payload_match
            .as_ref()
            .map(|marker| match marker {
                RawContextMatch::Name(name) => Ok(ContextMatch {
                    name: normalize_context_marker(syntax, name)?,
                    capture: None,
                }),
                RawContextMatch::Capture { name, capture } => Ok(ContextMatch {
                    name: normalize_context_marker(syntax, name)?,
                    capture: Some(*capture),
                }),
            })
            .transpose()?,
    }))
}

fn compile_injection_selector(
    syntax: &str,
    selector: RawInjectionSelector,
) -> Result<InjectedSyntaxSelector, SyntaxLoadError> {
    match selector {
        RawInjectionSelector::Static { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(SyntaxLoadError::MissingInjectedSyntaxSelector {
                    syntax: syntax.to_string(),
                });
            }
            Ok(InjectedSyntaxSelector::Static {
                name: SmolStr::new(name),
            })
        }
        RawInjectionSelector::Capture { capture } => {
            let capture = capture.trim();
            if capture.is_empty() {
                return Err(SyntaxLoadError::MissingInjectedSyntaxSelector {
                    syntax: syntax.to_string(),
                });
            }
            let pattern = Regex::new(capture).map_err(|error| SyntaxLoadError::InvalidRegex {
                syntax: syntax.to_string(),
                pattern: capture.to_string(),
                message: error.to_string(),
            })?;
            Ok(InjectedSyntaxSelector::Capture { pattern })
        }
    }
}

fn parse_tag(syntax: &str, tag: &str) -> Result<Tag, SyntaxLoadError> {
    Tag::parse(tag).map_err(|_| SyntaxLoadError::InvalidTag {
        syntax: syntax.to_string(),
        tag: tag.to_string(),
    })
}

fn resolve_glyph_color(syntax: &str, color: &RawGlyphColor) -> Result<Color, SyntaxLoadError> {
    match color {
        RawGlyphColor::Ansi(ansi) => Ok(Color::ansi(*ansi)),
        RawGlyphColor::Rgb(value) => parse_rgb_color(syntax, value),
    }
}

fn parse_rgb_color(syntax: &str, value: &str) -> Result<Color, SyntaxLoadError> {
    Rgb::parse_hex(value.trim())
        .map(Color::Rgb)
        .map_err(|_| SyntaxLoadError::InvalidGlyphColor {
            syntax: syntax.to_string(),
            color: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_injection_compiles_with_fallback() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: Some("//".to_string()),
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: vec![RawRule::Injection {
                selector: RawInjectionSelector::Capture {
                    capture: "^([A-Za-z]+)".to_string(),
                },
                fallback: RawInjectedSyntaxFallback::Unstyled,
                context: None,
            }],
        };

        let definition = resolve_syntax(raw, "example.toml").expect("injection should compile");
        let SyntaxRule::Injection {
            selector: InjectedSyntaxSelector::Capture { .. },
            fallback: InjectedSyntaxFallback::Unstyled,
            ..
        } = &definition.rules[0]
        else {
            panic!("expected injected rule");
        };
    }

    #[test]
    fn context_control_compiles_with_normalized_markers() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: vec![RawRule::Regex {
                pattern: "foo".to_string(),
                lookahead: None,
                tag: "string".to_string(),
                context: Some(RawContextControl {
                    requires: vec!["Context".to_string()],
                    push: vec![RawContextPush::Name("Next".to_string())],
                    pop: vec!["Context".to_string()],
                    payload_match: None,
                }),
            }],
        };

        let definition = resolve_syntax(raw, "example.toml").expect("region should compile");
        let SyntaxRule::Regex {
            context: Some(control),
            ..
        } = &definition.rules[0]
        else {
            panic!("expected context control");
        };

        assert_eq!(control.requires, vec!["context"]);
        assert_eq!(control.push.len(), 1);
        assert_eq!(control.push[0].name, "next");
        assert!(control.push[0].capture.is_none());
        assert_eq!(control.pop, vec!["context"]);
    }

    #[test]
    fn context_control_compiles_payload_matches() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: vec![RawRule::Regex {
                pattern: "foo".to_string(),
                lookahead: None,
                tag: "variable".to_string(),
                context: Some(RawContextControl {
                    requires: vec!["Context".to_string()],
                    push: vec![RawContextPush::Capture {
                        name: "Heredoc".to_string(),
                        capture: 1,
                    }],
                    pop: vec!["Context".to_string()],
                    payload_match: Some(RawContextMatch::Capture {
                        name: "Heredoc".to_string(),
                        capture: 1,
                    }),
                }),
            }],
        };

        let definition = resolve_syntax(raw, "example.toml").expect("region should compile");
        let SyntaxRule::Regex {
            context: Some(control),
            ..
        } = &definition.rules[0]
        else {
            panic!("expected context control");
        };

        assert_eq!(control.push.len(), 1);
        assert_eq!(control.push[0].name, "heredoc");
        assert_eq!(control.push[0].capture, Some(1));
        let payload_match = control
            .payload_match
            .as_ref()
            .expect("expected payload match");
        assert_eq!(payload_match.name, "heredoc");
        assert_eq!(payload_match.capture, Some(1));
    }

    #[test]
    fn regex_rule_compiles_lookahead() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: vec![RawRule::Regex {
                pattern: r"\bname\b".to_string(),
                lookahead: Some(r"\s*\(".to_string()),
                tag: "function".to_string(),
                context: None,
            }],
        };

        let definition = resolve_syntax(raw, "example.toml").expect("rule should compile");
        let SyntaxRule::Regex {
            lookahead: Some(lookahead),
            ..
        } = &definition.rules[0]
        else {
            panic!("expected lookahead");
        };

        assert_eq!(lookahead.as_str(), r"\s*\(");
    }

    #[test]
    fn metadata_compiles_comment_prefix() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: Some(" // ".to_string()),
                glyph: Some("".to_string()),
                glyph_color: Some(RawGlyphColor::Rgb("#dea584".to_string())),
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: Vec::new(),
        };

        let definition = resolve_syntax(raw, "example.toml").expect("metadata should compile");
        assert_eq!(definition.metadata.comment_prefix.as_deref(), Some("//"));
        assert_eq!(definition.metadata.glyph.as_deref(), Some(""));
        assert_eq!(
            definition.metadata.glyph_color,
            Some(Color::rgb(222, 165, 132))
        );
    }

    #[test]
    fn metadata_rejects_empty_comment_prefix() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: Some("   ".to_string()),
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: Vec::new(),
        };

        let error = resolve_syntax(raw, "example.toml").expect_err("empty prefix should fail");
        assert!(matches!(
            error,
            SyntaxLoadError::InvalidCommentPrefix { .. }
        ));
    }

    #[test]
    fn metadata_rejects_empty_glyph() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: Some("   ".to_string()),
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: Vec::new(),
        };

        let error = resolve_syntax(raw, "example.toml").expect_err("empty glyph should fail");
        assert!(matches!(error, SyntaxLoadError::InvalidGlyph { .. }));
    }

    #[test]
    fn metadata_rejects_invalid_glyph_color() {
        let raw = RawSyntaxDefinition {
            metadata: RawSyntaxMetadata {
                name: "example".to_string(),
                display_name: "Example".to_string(),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: Some("".to_string()),
                glyph_color: Some(RawGlyphColor::Rgb("not-a-color".to_string())),
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: Vec::new(),
        };

        let error =
            resolve_syntax(raw, "example.toml").expect_err("invalid glyph color should fail");
        assert!(matches!(error, SyntaxLoadError::InvalidGlyphColor { .. }));
    }
}
