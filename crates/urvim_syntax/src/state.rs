//! Shared syntax tokenizer state types and injection helpers.
//!
//! These types form the vocabulary between the syntax registry/tokenizers and
//! the buffer-side syntax cache. They live here (rather than in `buffer/syntax`)
//! so that the tokenizer modules can depend on them without creating a circular
//! `buffer` ↔ `syntax` dependency.

use crate::{SyntaxDefinition, SyntaxTokenizer, builtin_syntax_registry};
use std::collections::BTreeMap;
use std::sync::Arc;
use urvim_theme::Tag;

const CONTEXT_ID_OFFSET: u64 = 0xcbf29ce484222325;
const CONTEXT_ID_PRIME: u64 = 0x100000001b3;

/// Stable identity for one syntax context stack entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ContextId(u64);

impl ContextId {
    /// Creates a compile-time context id from a syntax namespace and local name.
    pub const fn new(namespace: &str, name: &str) -> Self {
        Self(hash_context_id(namespace, name))
    }
}

const fn hash_context_id(namespace: &str, name: &str) -> u64 {
    let mut hash = CONTEXT_ID_OFFSET;
    let namespace_bytes = namespace.as_bytes();
    let mut index = 0;
    while index < namespace_bytes.len() {
        hash ^= namespace_bytes[index] as u64;
        hash = hash.wrapping_mul(CONTEXT_ID_PRIME);
        index += 1;
    }

    hash ^= 0xff;
    hash = hash.wrapping_mul(CONTEXT_ID_PRIME);

    let name_bytes = name.as_bytes();
    index = 0;
    while index < name_bytes.len() {
        hash ^= name_bytes[index] as u64;
        hash = hash.wrapping_mul(CONTEXT_ID_PRIME);
        index += 1;
    }

    hash
}

/// A highlighted span within one buffer line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSpan {
    /// Start byte offset within the line.
    pub start_byte: usize,
    /// End byte offset within the line.
    pub end_byte: usize,
    /// The syntax tag that should style this span.
    pub style: Tag,
}

impl SyntaxSpan {
    /// Creates a new syntax span.
    pub fn new(start_byte: usize, end_byte: usize, style: Tag) -> Self {
        Self {
            start_byte,
            end_byte,
            style,
        }
    }
}

/// Tokenizer-scoped identifier used to match syntax fold open and close events.
pub type SyntaxFoldKind = u32;

/// Direction of a fold event produced by a tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxFoldEventKind {
    /// Opens a new fold region.
    Open,
    /// Closes the nearest matching open fold region.
    Close,
}

/// A line-based fold event emitted by a tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxFoldEvent {
    /// Whether this event opens or closes a fold region.
    pub kind: SyntaxFoldEventKind,
    /// Tokenizer-scoped kind used to match opens with closes.
    pub fold_kind: SyntaxFoldKind,
    /// Number of lines before the event line where a close should end.
    pub close_line_offset: usize,
}

impl SyntaxFoldEvent {
    /// Creates a new syntax fold event.
    pub fn new(kind: SyntaxFoldEventKind, fold_kind: SyntaxFoldKind) -> Self {
        Self {
            kind,
            fold_kind,
            close_line_offset: 0,
        }
    }

    /// Creates a close event whose region ends before the event line.
    pub fn close_before_current_line(fold_kind: SyntaxFoldKind) -> Self {
        Self {
            kind: SyntaxFoldEventKind::Close,
            fold_kind,
            close_line_offset: 1,
        }
    }
}

/// Result of tokenizing one line with a builtin scanner.
#[derive(Debug, Clone)]
pub struct SyntaxLineResult {
    /// Highlighted spans for the line.
    pub spans: Vec<SyntaxSpan>,
    /// Fold events emitted while scanning the line.
    pub fold_events: Vec<SyntaxFoldEvent>,
    /// State to pass to the next line.
    pub state: SyntaxState,
}

impl From<(Vec<SyntaxSpan>, SyntaxState)> for SyntaxLineResult {
    fn from((spans, state): (Vec<SyntaxSpan>, SyntaxState)) -> Self {
        Self {
            spans,
            fold_events: Vec::new(),
            state,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SyntaxState {
    #[default]
    Plain,
    Code(CodeState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeState {
    Scanner {
        contexts: ContextStack,
        injection: Option<TokenizerInjectionState>,
        parent_style: Option<Tag>,
        tokenizer_state: SyntaxTokenizerState,
    },
}

/// Opaque tokenizer-owned state carried between tokenized lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTokenizerState {
    values: BTreeMap<String, SyntaxTokenizerStateValue>,
}

impl SyntaxTokenizerState {
    /// Returns a stored unsigned integer value.
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        match self.values.get(key) {
            Some(SyntaxTokenizerStateValue::U32(value)) => Some(*value),
            _ => None,
        }
    }

    /// Sets or removes a stored unsigned integer value.
    pub fn set_u32(&mut self, key: impl Into<String>, value: Option<u32>) {
        let key = key.into();
        if let Some(value) = value {
            self.values
                .insert(key, SyntaxTokenizerStateValue::U32(value));
        } else {
            self.values.remove(&key);
        }
    }
}

/// IPC-friendly scalar value carried in tokenizer-owned state.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxTokenizerStateValue {
    /// Boolean state value.
    Bool(bool),
    /// Unsigned 32-bit integer state value.
    U32(u32),
    /// String state value.
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedSyntaxFallback {
    Unstyled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizerInjectionState {
    pub nested: Option<NestedState>,
    pub fallback: InjectedSyntaxFallback,
    pub parent_style: Option<Tag>,
}

#[derive(Debug, Clone)]
pub enum NestedState {
    Syntax {
        syntax_definition: Arc<SyntaxDefinition>,
        state: Box<SyntaxState>,
    },
}

impl NestedState {
    pub fn new_syntax(definition: Arc<SyntaxDefinition>) -> Self {
        let state = Box::new(initial_state_for_definition(definition.as_ref()));
        NestedState::Syntax {
            syntax_definition: definition,
            state,
        }
    }
}

impl PartialEq for NestedState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Syntax {
                    syntax_definition: left_definition,
                    state: left_state,
                },
                Self::Syntax {
                    syntax_definition: right_definition,
                    state: right_state,
                },
            ) => left_definition.name() == right_definition.name() && left_state == right_state,
        }
    }
}

impl Eq for NestedState {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextStack {
    pub entries: Vec<ContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    pub name: ContextId,
    pub payload: Option<String>,
}

impl ContextStack {
    pub fn top_is(&self, marker: ContextId) -> bool {
        self.entries
            .last()
            .is_some_and(|active| active.name == marker)
    }

    pub fn contains_anywhere(&self, marker: ContextId) -> bool {
        self.depth(marker) > 0
    }

    pub fn depth(&self, marker: ContextId) -> usize {
        self.entries
            .iter()
            .filter(|active| active.name == marker)
            .count()
    }

    pub fn payload_for(&self, name: ContextId) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.payload.as_deref())
    }

    pub fn push(&mut self, marker: ContextId) {
        self.entries.push(ContextEntry {
            name: marker,
            payload: None,
        });
    }

    pub fn pop(&mut self, marker: ContextId) {
        if let Some(index) = self
            .entries
            .iter()
            .rposition(|active| active.name == marker)
        {
            self.entries.remove(index);
        }
    }

    pub fn pop_top(&mut self, marker: ContextId) -> bool {
        if self.top_is(marker) {
            self.entries.pop();
            true
        } else {
            false
        }
    }

    pub fn push_with_payload(&mut self, marker: ContextId, payload: &str) {
        self.entries.push(ContextEntry {
            name: marker,
            payload: Some(payload.to_string()),
        });
    }
}

pub fn syntax_definition(syntax_name: &str) -> Option<Arc<SyntaxDefinition>> {
    builtin_syntax_registry().ok()?.get_by_name(syntax_name)
}

pub fn syntax_definition_by_name(name: &str) -> Option<Arc<SyntaxDefinition>> {
    syntax_definition(name)
}

pub fn tokenize_line_definition(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> SyntaxLineResult {
    crate::tokenizers::dispatch_builtin(definition.tokenizer, definition, line, state)
}

pub fn tokenize_nested_body(
    nested: &mut NestedState,
    body: &str,
    offset: usize,
) -> Vec<SyntaxSpan> {
    match nested {
        NestedState::Syntax {
            syntax_definition,
            state,
        } => {
            let SyntaxLineResult {
                spans,
                fold_events: _,
                state: next_state,
            } = tokenize_line_definition(syntax_definition.as_ref(), body, state.as_ref().clone());
            **state = next_state;
            offset_spans(spans, offset)
        }
    }
}

/// Tokenize injected nested syntax and keep the host parent style in sync.
pub fn tokenize_injected_body(
    inj: &mut TokenizerInjectionState,
    body: &str,
    offset: usize,
) -> Vec<SyntaxSpan> {
    if let Some(ref mut nested) = inj.nested {
        let spans = tokenize_nested_body(nested, body, offset);
        let NestedState::Syntax { state, .. } = nested;
        if let SyntaxState::Code(CodeState::Scanner {
            parent_style: ps, ..
        }) = state.as_ref()
        {
            inj.parent_style = ps.clone();
        }
        spans
    } else {
        match inj.fallback {
            InjectedSyntaxFallback::Unstyled => Vec::new(),
        }
    }
}

fn offset_spans(spans: Vec<SyntaxSpan>, offset: usize) -> Vec<SyntaxSpan> {
    spans
        .into_iter()
        .map(|span| SyntaxSpan::new(span.start_byte + offset, span.end_byte + offset, span.style))
        .collect()
}

pub fn initial_state_for_definition(definition: &SyntaxDefinition) -> SyntaxState {
    match definition.tokenizer {
        SyntaxTokenizer::Plaintext => SyntaxState::Plain,
        _ => SyntaxState::Code(CodeState::Scanner {
            contexts: ContextStack::default(),
            injection: None,
            parent_style: None,
            tokenizer_state: SyntaxTokenizerState::default(),
        }),
    }
}
