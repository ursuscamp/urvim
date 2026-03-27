//! Closed sets of UI and syntax style keys.

/// Predefined UI style keys supported by urvim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiStyleKey {
    /// The editor status bar.
    StatusBar,
    /// The currently active tab.
    TabActive,
    /// A non-active tab.
    TabInactive,
    /// A scroll indicator shown in the tab bar.
    TabScrollIndicator,
    /// The gutter beside the buffer text.
    Gutter,
    /// The main buffer viewport.
    Window,
}

/// Predefined syntax style keys supported by urvim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxStyleKey {
    /// Comments and documentation strings.
    Comment,
    /// Constants and immutable symbols.
    Constant,
    /// Function names.
    Function,
    /// Keywords and control flow.
    Keyword,
    /// Numeric literals.
    Number,
    /// Operators such as `+` and `=`.
    Operator,
    /// Punctuation such as commas and braces.
    Punctuation,
    /// String literals.
    String,
    /// Type names and declarations.
    Type,
    /// General variable names.
    Variable,
}
