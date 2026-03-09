//! Key codes, modifiers, and input events for terminal input handling.
//!
//! This module defines the types used to represent keyboard input from the terminal,
//! including key codes, modifier states (Shift, Alt, Ctrl, etc.), and the unified
//! `Event` type that can represent key presses, terminal resizes, and paste events.

#[allow(dead_code)]
/// Keyboard modifier state flags.
///
/// Multiple modifiers can be combined using the bitwise OR operator (`|`).
///
/// # Example
///
/// ```
/// use urvim::terminal::Modifiers;
///
/// let modifiers = Modifiers::SHIFT | Modifiers::CTRL;
/// assert!(modifiers.has_shift());
/// assert!(modifiers.has_ctrl());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers(u8);

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[allow(dead_code)]
impl Modifiers {
    /// Shift key modifier (bit 0).
    pub const SHIFT: Self = Self(0b1);
    /// Alt key modifier (bit 1).
    pub const ALT: Self = Self(0b10);
    /// Ctrl key modifier (bit 2).
    pub const CTRL: Self = Self(0b100);
    /// Super/Win/Cmd key modifier (bit 3).
    pub const SUPER: Self = Self(0b1000);
    /// Hyper key modifier (bit 4).
    pub const HYPER: Self = Self(0b10000);
    /// Meta key modifier (bit 5).
    pub const META: Self = Self(0b100000);

    /// Returns `true` if the Shift modifier is active.
    pub fn has_shift(self) -> bool {
        self.0 & 1 != 0
    }

    /// Returns `true` if the Alt modifier is active.
    pub fn has_alt(self) -> bool {
        self.0 & 2 != 0
    }

    /// Returns `true` if the Ctrl modifier is active.
    pub fn has_ctrl(self) -> bool {
        self.0 & 4 != 0
    }

    /// Returns `true` if the Super modifier is active.
    pub fn has_super(self) -> bool {
        self.0 & 8 != 0
    }

    /// Returns `true` if the Hyper modifier is active.
    pub fn has_hyper(self) -> bool {
        self.0 & 16 != 0
    }

    /// Returns `true` if the Meta modifier is active.
    pub fn has_meta(self) -> bool {
        self.0 & 32 != 0
    }

    /// Converts a Kitty keyboard protocol encoding to `Modifiers`.
    ///
    /// The Kitty protocol uses values 0-7 where:
    /// - 0: no modifiers
    /// - 2: Shift
    /// - 3: Alt
    /// - 4: Ctrl
    /// - 5: Ctrl+Shift
    /// - 6: Alt+Ctrl
    /// - 7: Alt+Ctrl+Shift
    ///
    /// This function subtracts 1 from the value to convert to the internal bitmask.
    pub fn from_kitty_encoding(value: u8) -> Self {
        if value == 0 {
            return Self::default();
        }
        Self(value.saturating_sub(1))
    }
}

/// Represents a virtual key code from the terminal.
///
/// This enum covers standard keys (letters, numbers, punctuation),
/// control keys (Enter, Tab, Backspace, Escape), navigation keys
/// (arrow keys, Home, End, Page Up/Down), and function keys (F1-F12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A character key (Unicode code point).
    Char(char),
    /// Enter/Return key.
    Enter,
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Tab key.
    Tab,
    /// Escape key.
    Esc,
    /// Up arrow key.
    Up,
    /// Down arrow key.
    Down,
    /// Left arrow key.
    Left,
    /// Right arrow key.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page Up key.
    PageUp,
    /// Page Down key.
    PageDown,
    /// Function key F1.
    F1,
    /// Function key F2.
    F2,
    /// Function key F3.
    F3,
    /// Function key F4.
    F4,
    /// Function key F5.
    F5,
    /// Function key F6.
    F6,
    /// Function key F7.
    F7,
    /// Function key F8.
    F8,
    /// Function key F9.
    F9,
    /// Function key F10.
    F10,
    /// Function key F11.
    F11,
    /// Function key F12.
    F12,
    /// Insert key.
    Insert,
    /// Null key (typically from empty input).
    Null,
}

#[allow(dead_code)]
impl KeyCode {
    /// Creates a new `Key` with this key code and no modifiers.
    pub fn key(self) -> Key {
        Key::new(self)
    }

    /// Creates a new `Event::Key` with this key code and no modifiers.
    pub fn event(self) -> Event {
        Event::Key(Key::new(self))
    }

    /// Creates a new `Key` with this key code and the specified modifiers.
    pub fn with_modifiers(self, modifiers: Modifiers) -> Key {
        Key::with_modifiers(self, modifiers)
    }
}

/// A key press consisting of a key code and optional modifiers.
///
/// This represents a complete key event, combining the virtual key code
/// with any modifier keys (Shift, Alt, Ctrl, etc.) that were pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The key code of this key press.
    pub code: KeyCode,
    /// The modifier keys held during this key press.
    pub modifiers: Modifiers,
}

impl Key {
    /// Creates a new key with the specified key code and no modifiers.
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::default(),
        }
    }

    /// Creates a new key with the specified key code and modifiers.
    pub fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }

    /// Wraps this key in an `Event::Key`.
    pub fn event(self) -> Event {
        Event::Key(self)
    }
}

/// Terminal input events.
///
/// This enum represents all possible types of input events that can be
/// received from the terminal:
/// - Key presses (with optional modifiers)
/// - Terminal resize events
/// - Bracketed paste events (large text pastes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A key press event.
    Key(Key),
    /// A terminal resize event (rows, columns).
    Resize(u16, u16),
    /// A bracketed paste event containing pasted text.
    Paste(String),
}
