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
/// use urvim_terminal::Modifiers;
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

    /// Returns `true` when no modifiers are active.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

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
    /// The Kitty protocol uses values where:
    /// - 0: no modifiers
    /// - 2: Shift (1+1)
    /// - 3: Alt (1+2)
    /// - 5: Ctrl (1+4)
    /// - 6: Ctrl+Shift (1+4+1)
    /// - 7: Alt+Ctrl (1+2+4)
    /// - 8: Alt+Ctrl+Shift (1+2+4+1)
    /// - And higher values for Super, Hyper, Meta combinations
    ///
    /// Invalid values (1, 4) return no modifiers to avoid incorrect behavior.
    pub fn from_kitty_encoding(value: u8) -> Self {
        // Value 1 is invalid (would be 1+0 = no modifiers encoded, but that's ambiguous)
        // Value 4 is invalid (would be 1+3, which is not a valid modifier combination)
        if value == 0 || value == 1 || value == 4 {
            return Self::default();
        }
        Self(value.saturating_sub(1))
    }

    /// Returns the modifier prefixes in canonical order: Ctrl → Alt → Shift → Super → Hyper → Meta.
    pub fn to_prefixes(self) -> Vec<&'static str> {
        let mut prefixes = Vec::new();
        if self.has_ctrl() {
            prefixes.push("C");
        }
        if self.has_alt() {
            prefixes.push("A");
        }
        if self.has_shift() {
            prefixes.push("S");
        }
        if self.has_super() {
            prefixes.push("Su");
        }
        if self.has_hyper() {
            prefixes.push("H");
        }
        if self.has_meta() {
            prefixes.push("M");
        }
        prefixes
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
    /// Menu/Application/Context menu key.
    Menu,
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

    /// Returns the special name for this key code, or `None` if it's a character key.
    ///
    /// For example, `KeyCode::Enter` returns `Some("Enter")`, but `KeyCode::Char('a')` returns `None`.
    pub fn special_name(&self) -> Option<&'static str> {
        match self {
            KeyCode::Char(_) => None,
            KeyCode::Enter => Some("Enter"),
            KeyCode::Backspace => Some("Backspace"),
            KeyCode::Delete => Some("Delete"),
            KeyCode::Tab => Some("Tab"),
            KeyCode::Esc => Some("Esc"),
            KeyCode::Up => Some("Up"),
            KeyCode::Down => Some("Down"),
            KeyCode::Left => Some("Left"),
            KeyCode::Right => Some("Right"),
            KeyCode::Home => Some("Home"),
            KeyCode::End => Some("End"),
            KeyCode::PageUp => Some("PageUp"),
            KeyCode::PageDown => Some("PageDown"),
            KeyCode::F1 => Some("F1"),
            KeyCode::F2 => Some("F2"),
            KeyCode::F3 => Some("F3"),
            KeyCode::F4 => Some("F4"),
            KeyCode::F5 => Some("F5"),
            KeyCode::F6 => Some("F6"),
            KeyCode::F7 => Some("F7"),
            KeyCode::F8 => Some("F8"),
            KeyCode::F9 => Some("F9"),
            KeyCode::F10 => Some("F10"),
            KeyCode::F11 => Some("F11"),
            KeyCode::F12 => Some("F12"),
            KeyCode::Insert => Some("Insert"),
            KeyCode::Menu => Some("Menu"),
            KeyCode::Null => Some("Null"),
        }
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

    /// Returns a canonical string representation of this key.
    ///
    /// The canonical representation follows these rules:
    /// - Printable characters (including emojis) are returned as-is, except for:
    ///   - Space (`' '`) → `<Space>`
    ///   - Less-than (`'<'`) → `<LessThan>`
    ///   - Greater-than (`'>'`) → `<GreaterThan>`
    /// - Special keys use angle bracket notation: `<Enter>`, `<Up>`, `<F1>`, etc.
    /// - Modifier combinations use the format `<M-key>` where M is the modifier prefix
    /// - Modifier order is: Ctrl → Alt → Shift → Super → Hyper → Meta
    /// - Shift + letter is normalized to uppercase (e.g., Shift+a → "A")
    /// - Shift + number/punctuation returns the shifted character (e.g., Shift+1 → "!")
    pub fn canonical_string(&self) -> String {
        // Handle character keys
        if let KeyCode::Char(c) = self.code {
            // Handle modifiers first - if any modifier is present, use modifier notation
            let has_shift = self.modifiers.has_shift();
            let has_ctrl = self.modifiers.has_ctrl();
            let has_other_modifier = self.modifiers.has_alt()
                || self.modifiers.has_super()
                || self.modifiers.has_hyper()
                || self.modifiers.has_meta();

            // If Ctrl or other modifiers are present (or Shift on special keys)
            if has_ctrl || has_other_modifier || has_shift {
                // If only Shift is pressed on a letter, normalize to uppercase
                if has_shift && !has_ctrl && !has_other_modifier && c.is_ascii_alphabetic() {
                    return c.to_ascii_uppercase().to_string();
                }

                // If only Shift is pressed on a character with shifted representation
                if has_shift
                    && !has_ctrl
                    && !has_other_modifier
                    && let Some(shifted) = get_shifted_char(c)
                {
                    return shifted.to_string();
                }

                // Otherwise use modifier notation
                let prefixes = self.modifiers.to_prefixes();
                let prefix_str = prefixes.join("-");
                return format!("<{}-{}>", prefix_str, c);
            }

            // No modifiers - check for special exception characters: space, <, >
            if c == ' ' {
                return "<Space>".to_string();
            }
            if c == '<' {
                return "<LessThan>".to_string();
            }
            if c == '>' {
                return "<GreaterThan>".to_string();
            }

            // No modifiers - return character as-is
            return c.to_string();
        }

        // Handle special keys (non-character KeyCode)
        if let Some(special_name) = self.code.special_name() {
            let has_any_modifier = self.modifiers.has_ctrl()
                || self.modifiers.has_alt()
                || self.modifiers.has_shift()
                || self.modifiers.has_super()
                || self.modifiers.has_hyper()
                || self.modifiers.has_meta();

            if has_any_modifier {
                let prefixes = self.modifiers.to_prefixes();
                let prefix_str = prefixes.join("-");
                return format!("<{}-{}>", prefix_str, special_name);
            }

            return format!("<{}>", special_name);
        }

        // Fallback (should not reach here for valid KeyCode)
        "<Unknown>".to_string()
    }
}

/// Returns the shifted character for a given key on a US keyboard layout.
///
/// Returns `None` if there is no shifted representation for the character.
fn get_shifted_char(c: char) -> Option<char> {
    match c {
        '`' => Some('~'),
        '1' => Some('!'),
        '2' => Some('@'),
        '3' => Some('#'),
        '4' => Some('$'),
        '5' => Some('%'),
        '6' => Some('^'),
        '7' => Some('&'),
        '8' => Some('*'),
        '9' => Some('('),
        '0' => Some(')'),
        '-' => Some('_'),
        '=' => Some('+'),
        '[' => Some('{'),
        ']' => Some('}'),
        '\\' => Some('|'),
        ';' => Some(':'),
        '\'' => Some('"'),
        ',' => Some('<'),
        '.' => Some('>'),
        '/' => Some('?'),
        _ => None,
    }
}

/// Terminal input events.
///
/// This enum represents all possible types of input events that can be
/// received from the terminal:
/// - Key presses (with optional modifiers)
/// - Terminal resize events
/// - Timer ticks used to wake the editor loop for background work
/// - Bracketed paste events (large text pastes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A key press event.
    Key(Key),
    /// A terminal resize event (rows, columns).
    Resize(u16, u16),
    /// A timer tick emitted when the terminal poll times out.
    Tick,
    /// A bracketed paste event containing pasted text.
    Paste(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Basic Characters ====================

    #[test]
    fn test_lowercase_letters() {
        for c in 'a'..='z' {
            let key = Key::new(KeyCode::Char(c));
            assert_eq!(key.canonical_string(), c.to_string(), "Failed for '{}'", c);
        }
    }

    #[test]
    fn test_uppercase_letters() {
        for c in 'A'..='Z' {
            let key = Key::new(KeyCode::Char(c));
            assert_eq!(key.canonical_string(), c.to_string(), "Failed for '{}'", c);
        }
    }

    #[test]
    fn test_digits() {
        for c in '0'..='9' {
            let key = Key::new(KeyCode::Char(c));
            assert_eq!(key.canonical_string(), c.to_string(), "Failed for '{}'", c);
        }
    }

    #[test]
    fn test_punctuation() {
        // Note: < and > are special exceptions that return <LessThan> and <GreaterThan>
        // We test them separately in test_less_than_character and test_greater_than_character
        let punctuation = "!@#$%^&*()_+-=[]{}|;':\",./?\\`";
        for c in punctuation.chars() {
            let key = Key::new(KeyCode::Char(c));
            assert_eq!(key.canonical_string(), c.to_string(), "Failed for '{}'", c);
        }
    }

    #[test]
    fn test_unicode_characters() {
        // Emoji
        let key = Key::new(KeyCode::Char('😀'));
        assert_eq!(key.canonical_string(), "😀");

        // Wide character
        let key = Key::new(KeyCode::Char('日'));
        assert_eq!(key.canonical_string(), "日");

        // Other Unicode
        let key = Key::new(KeyCode::Char('é'));
        assert_eq!(key.canonical_string(), "é");
    }

    // ==================== Special Exceptions ====================

    #[test]
    fn test_space_character() {
        let key = Key::new(KeyCode::Char(' '));
        assert_eq!(key.canonical_string(), "<Space>");
    }

    #[test]
    fn test_less_than_character() {
        let key = Key::new(KeyCode::Char('<'));
        assert_eq!(key.canonical_string(), "<LessThan>");
    }

    #[test]
    fn test_greater_than_character() {
        let key = Key::new(KeyCode::Char('>'));
        assert_eq!(key.canonical_string(), "<GreaterThan>");
    }

    // ==================== Modifiers Only ====================

    #[test]
    fn test_ctrl_with_letters() {
        for c in ['a', 'b', 'c', 'x', 'z'] {
            let key = Key::with_modifiers(KeyCode::Char(c), Modifiers::CTRL);
            assert_eq!(key.canonical_string(), format!("<C-{}>", c));
        }
    }

    #[test]
    fn test_alt_with_letters() {
        let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::ALT);
        assert_eq!(key.canonical_string(), "<A-a>");
    }

    #[test]
    fn test_shift_normalizes_letters_to_uppercase() {
        for c in 'a'..='z' {
            let key = Key::with_modifiers(KeyCode::Char(c), Modifiers::SHIFT);
            assert_eq!(
                key.canonical_string(),
                c.to_ascii_uppercase().to_string(),
                "Failed for '{}'",
                c
            );
        }
    }

    #[test]
    fn test_shift_normalizes_uppercase_letters() {
        for c in 'A'..='Z' {
            let key = Key::with_modifiers(KeyCode::Char(c), Modifiers::SHIFT);
            assert_eq!(key.canonical_string(), c.to_string());
        }
    }

    #[test]
    fn test_modifier_order_ctrl_alt() {
        let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::ALT);
        // Canonical order: Ctrl → Alt → Shift, so Ctrl first
        assert_eq!(key.canonical_string(), "<C-A-a>");
    }

    #[test]
    fn test_modifier_order_ctrl_alt_shift() {
        let key = Key::with_modifiers(
            KeyCode::Char('a'),
            Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
        );
        // Canonical order: Ctrl → Alt → Shift
        assert_eq!(key.canonical_string(), "<C-A-S-a>");
    }

    #[test]
    fn test_super_modifier() {
        let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::SUPER);
        assert_eq!(key.canonical_string(), "<Su-a>");
    }

    #[test]
    fn test_hyper_modifier() {
        let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::HYPER);
        assert_eq!(key.canonical_string(), "<H-a>");
    }

    #[test]
    fn test_meta_modifier() {
        let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::META);
        assert_eq!(key.canonical_string(), "<M-a>");
    }

    // ==================== Shift + Number/Punctuation ====================

    #[test]
    fn test_shift_number_row() {
        let shifts = [
            ('1', '!'),
            ('2', '@'),
            ('3', '#'),
            ('4', '$'),
            ('5', '%'),
            ('6', '^'),
            ('7', '&'),
            ('8', '*'),
            ('9', '('),
            ('0', ')'),
        ];
        for (base, shifted) in shifts {
            let key = Key::with_modifiers(KeyCode::Char(base), Modifiers::SHIFT);
            assert_eq!(
                key.canonical_string(),
                shifted.to_string(),
                "Failed for Shift+{}",
                base
            );
        }
    }

    #[test]
    fn test_shift_punctuation() {
        let shifts = [
            ('-', '_'),
            ('=', '+'),
            ('[', '{'),
            (']', '}'),
            ('\\', '|'),
            (';', ':'),
            ('\'', '"'),
            (',', '<'),
            ('.', '>'),
            ('/', '?'),
            ('`', '~'),
        ];
        for (base, shifted) in shifts {
            let key = Key::with_modifiers(KeyCode::Char(base), Modifiers::SHIFT);
            assert_eq!(
                key.canonical_string(),
                shifted.to_string(),
                "Failed for Shift+{}",
                base
            );
        }
    }

    // ==================== Special Keys ====================

    #[test]
    fn test_navigation_keys() {
        let tests = [
            (KeyCode::Up, "<Up>"),
            (KeyCode::Down, "<Down>"),
            (KeyCode::Left, "<Left>"),
            (KeyCode::Right, "<Right>"),
            (KeyCode::Home, "<Home>"),
            (KeyCode::End, "<End>"),
            (KeyCode::PageUp, "<PageUp>"),
            (KeyCode::PageDown, "<PageDown>"),
        ];
        for (code, expected) in tests {
            let key = Key::new(code);
            assert_eq!(key.canonical_string(), expected, "Failed for {:?}", code);
        }
    }

    #[test]
    fn test_function_keys() {
        for i in 1..=12 {
            let code = match i {
                1 => KeyCode::F1,
                2 => KeyCode::F2,
                3 => KeyCode::F3,
                4 => KeyCode::F4,
                5 => KeyCode::F5,
                6 => KeyCode::F6,
                7 => KeyCode::F7,
                8 => KeyCode::F8,
                9 => KeyCode::F9,
                10 => KeyCode::F10,
                11 => KeyCode::F11,
                12 => KeyCode::F12,
                _ => unreachable!(),
            };
            let key = Key::new(code);
            assert_eq!(key.canonical_string(), format!("<F{}>", i));
        }
    }

    #[test]
    fn test_control_keys() {
        let tests = [
            (KeyCode::Enter, "<Enter>"),
            (KeyCode::Tab, "<Tab>"),
            (KeyCode::Backspace, "<Backspace>"),
            (KeyCode::Delete, "<Delete>"),
            (KeyCode::Insert, "<Insert>"),
            (KeyCode::Menu, "<Menu>"),
            (KeyCode::Esc, "<Esc>"),
            (KeyCode::Null, "<Null>"),
        ];
        for (code, expected) in tests {
            let key = Key::new(code);
            assert_eq!(key.canonical_string(), expected, "Failed for {:?}", code);
        }
    }

    #[test]
    fn test_shift_tab_canonical_string() {
        let key = Key::with_modifiers(KeyCode::Tab, Modifiers::SHIFT);
        assert_eq!(key.canonical_string(), "<S-Tab>");
    }

    // ==================== Special Keys with Modifiers ====================

    #[test]
    fn test_special_key_with_ctrl() {
        let key = Key::with_modifiers(KeyCode::Enter, Modifiers::CTRL);
        assert_eq!(key.canonical_string(), "<C-Enter>");
    }

    #[test]
    fn test_special_key_with_shift() {
        let key = Key::with_modifiers(KeyCode::Enter, Modifiers::SHIFT);
        assert_eq!(key.canonical_string(), "<S-Enter>");
    }

    #[test]
    fn test_special_key_with_ctrl_shift() {
        let key = Key::with_modifiers(KeyCode::Enter, Modifiers::CTRL | Modifiers::SHIFT);
        assert_eq!(key.canonical_string(), "<C-S-Enter>");
    }

    #[test]
    fn test_arrow_with_ctrl() {
        let key = Key::with_modifiers(KeyCode::Up, Modifiers::CTRL);
        assert_eq!(key.canonical_string(), "<C-Up>");
    }

    #[test]
    fn test_function_with_ctrl() {
        let key = Key::with_modifiers(KeyCode::F1, Modifiers::CTRL);
        assert_eq!(key.canonical_string(), "<C-F1>");
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_ctrl_with_special_characters() {
        // Ctrl+[ should work (Ctrl+[ is escape)
        let key = Key::with_modifiers(KeyCode::Char('['), Modifiers::CTRL);
        assert_eq!(key.canonical_string(), "<C-[>");
    }

    #[test]
    fn test_space_with_ctrl() {
        // Space with Ctrl should use special notation
        let key = Key::with_modifiers(KeyCode::Char(' '), Modifiers::CTRL);
        assert_eq!(key.canonical_string(), "<C- >");
    }

    #[test]
    fn test_modifiers_to_prefixes_order() {
        // Verify canonical order: Ctrl -> Alt -> Shift -> Super -> Hyper -> Meta
        let mods = Modifiers::CTRL
            | Modifiers::ALT
            | Modifiers::SHIFT
            | Modifiers::SUPER
            | Modifiers::HYPER
            | Modifiers::META;
        let prefixes = mods.to_prefixes();
        assert_eq!(prefixes, vec!["C", "A", "S", "Su", "H", "M"]);
    }
}
