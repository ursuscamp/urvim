use crate::theme::Tag;
use regex::Regex;
use serde::Deserialize;
use smol_str::SmolStr;

/// How unresolved injected syntax should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedSyntaxFallback {
    /// Render the body using the enclosing region style.
    ParentStyle,
    /// Render the body without nested syntax styling.
    Unstyled,
}

/// A nested syntax selector for an injected region.
#[derive(Debug, Clone)]
pub enum InjectedSyntaxSelector {
    /// Resolve a fixed canonical syntax name.
    Static { name: SmolStr },
    /// Resolve from a captured region-opener tag.
    Capture { pattern: Regex },
}

/// An injected syntax rule attached to a regex-driven body.
#[derive(Debug, Clone)]
pub struct InjectedSyntaxRule {
    /// How the nested syntax should be resolved.
    pub selector: InjectedSyntaxSelector,
    /// How unresolved nested syntax should render.
    pub fallback: InjectedSyntaxFallback,
}

/// Context-sensitive matching and stack updates for a syntax region.
#[derive(Debug, Clone)]
pub struct ContextControl {
    /// Context markers that must already be active for the region to match.
    pub requires: Vec<SmolStr>,
    /// Context markers to activate when the region opens.
    pub push: Vec<ContextPush>,
    /// Context markers to deactivate when the region opens or closes.
    pub pop: Vec<SmolStr>,
    /// Optional active context name and capture index whose payload must prefix-match the token text.
    pub payload_match: Option<ContextMatch>,
}

/// A context marker to activate, optionally populated from a regex capture.
#[derive(Debug, Clone)]
pub struct ContextPush {
    /// The active context name.
    pub name: SmolStr,
    /// Optional capture group index copied into the context payload.
    pub capture: Option<usize>,
}

/// A payload-bearing context match constraint.
#[derive(Debug, Clone)]
pub struct ContextMatch {
    /// The active context name.
    pub name: SmolStr,
    /// Optional capture group index to compare against the active payload.
    pub capture: Option<usize>,
}

/// An active context entry tracked by the syntax tokenizer.
#[derive(Debug, Clone)]
pub struct ContextEntry {
    /// The active context name.
    pub name: SmolStr,
    /// The payload associated with the active context, if any.
    pub payload: Option<String>,
}

/// A compiled syntax definition loaded from TOML.
#[derive(Debug, Clone)]
pub struct SyntaxDefinition {
    /// Syntax metadata used for resolution and display.
    pub metadata: SyntaxMetadata,
    /// Ordered rules used by the context-driven tokenizer.
    pub rules: Vec<SyntaxRule>,
}

/// A compiled syntax rule in the context-driven rule list.
#[derive(Debug, Clone)]
pub enum SyntaxRule {
    /// A regular-expression rule that emits a tag and can update context.
    Regex {
        /// Compiled regular expression.
        regex: Regex,
        /// Syntax tag applied to the match.
        tag: Tag,
        /// Context updates applied when this rule matches.
        context: Option<ContextControl>,
    },
    /// A nested-syntax delegation rule that uses active context to decide body highlighting.
    Injection {
        /// How the nested syntax should be resolved.
        selector: InjectedSyntaxSelector,
        /// How unresolved nested syntax should render.
        fallback: InjectedSyntaxFallback,
        /// Context updates applied when this rule matches.
        context: Option<ContextControl>,
    },
}

impl SyntaxDefinition {
    /// Returns the canonical syntax name.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns the user-facing syntax label.
    pub fn display_name(&self) -> &str {
        &self.metadata.display_name
    }

    /// Returns the ordered rules list, if the syntax uses the new context-driven model.
    pub fn rules(&self) -> &[SyntaxRule] {
        &self.rules
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
    /// Compiled filename regexes.
    pub filename: Vec<Regex>,
    /// Compiled shebang regexes.
    pub shebang: Vec<Regex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSyntaxDefinition {
    pub metadata: RawSyntaxMetadata,
    #[serde(default)]
    pub rules: Vec<RawRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSyntaxMetadata {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub alias: Vec<String>,
    #[serde(default)]
    pub comment_prefix: Option<String>,
    #[serde(default)]
    pub filename: Vec<String>,
    #[serde(default)]
    pub shebang: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RawRule {
    Regex {
        pattern: String,
        tag: String,
        #[serde(default)]
        context: Option<RawContextControl>,
    },
    Injection {
        selector: RawInjectionSelector,
        fallback: RawInjectedSyntaxFallback,
        #[serde(default)]
        context: Option<RawContextControl>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawContextControl {
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub push: Vec<RawContextPush>,
    #[serde(default)]
    pub pop: Vec<String>,
    #[serde(default)]
    pub payload_match: Option<RawContextMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(super) enum RawContextPush {
    Name(String),
    Capture { name: String, capture: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(super) enum RawContextMatch {
    Name(String),
    Capture { name: String, capture: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub(super) enum RawInjectionSelector {
    Static { name: String },
    Capture { capture: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RawInjectedSyntaxFallback {
    ParentStyle,
    Unstyled,
}
