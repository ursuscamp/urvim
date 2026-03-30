//! Closed sets of UI keys.

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
