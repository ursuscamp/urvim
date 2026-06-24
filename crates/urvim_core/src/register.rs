use crate::config::DefaultRegisters;
use std::collections::BTreeMap;

/// The logical target of a register operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterName(pub char);

impl RegisterName {
    /// The unnamed register (`""`). Automatically mirrors every yank, delete, and change
    /// operation, and is the default source for paste when no explicit register is specified.
    pub const UNNAMED: Self = Self('"');

    /// Creates a register name from a concrete selector character.
    pub fn new(selector: char) -> Self {
        Self(selector)
    }

    /// Returns the underlying selector character.
    pub fn as_char(self) -> char {
        self.0
    }
}

/// The stored content kind used to resolve paste placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterContentKind {
    /// Inline text copied from a characterwise selection.
    Characterwise,
    /// Whole-line text copied from a linewise selection.
    Linewise,
}

/// The text stored in a register along with its paste semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterContent {
    /// The exact text stored in the register.
    pub text: String,
    /// The selection granularity of the stored text.
    pub kind: RegisterContentKind,
}

impl RegisterContent {
    /// Creates new register content.
    pub fn new(text: String, kind: RegisterContentKind) -> Self {
        Self { text, kind }
    }
}

/// A simple session-local register store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterStore {
    entries: BTreeMap<RegisterName, RegisterContent>,
}

/// The built-in default register roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultRegisterRole {
    /// Yank destination.
    Yank,
    /// Delete destination.
    Delete,
    /// Change destination.
    Change,
}

impl RegisterName {
    /// Returns the register selected by the given prefix key and configured defaults.
    pub fn from_prefix(selector: char, defaults: &DefaultRegisters) -> Option<Self> {
        match selector {
            'y' => Some(default_register_name(DefaultRegisterRole::Yank, defaults)),
            'd' => Some(default_register_name(DefaultRegisterRole::Delete, defaults)),
            'c' => Some(default_register_name(DefaultRegisterRole::Change, defaults)),
            ch if ch.is_ascii_lowercase() => Some(Self::new(ch)),
            _ => None,
        }
    }
}

impl RegisterStore {
    /// Creates an empty register store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of the stored register content, if present.
    pub fn get(&self, name: RegisterName) -> Option<RegisterContent> {
        self.entries.get(&name).cloned()
    }

    /// Stores content in the given register.
    pub fn set(&mut self, name: RegisterName, content: RegisterContent) {
        self.entries.insert(name, content);
    }

    /// Removes any content stored in the given register.
    pub fn clear(&mut self, name: RegisterName) {
        self.entries.remove(&name);
    }
}

/// Resolves a built-in register role into the configured concrete register name.
pub fn default_register_name(
    role: DefaultRegisterRole,
    defaults: &DefaultRegisters,
) -> RegisterName {
    match role {
        DefaultRegisterRole::Yank => concrete_default_name(defaults.yank, 'y'),
        DefaultRegisterRole::Delete => concrete_default_name(defaults.delete, 'd'),
        DefaultRegisterRole::Change => concrete_default_name(defaults.change, 'c'),
    }
}

fn concrete_default_name(selector: char, builtin: char) -> RegisterName {
    if selector == builtin {
        RegisterName::new(builtin)
    } else {
        RegisterName::new(selector)
    }
}
