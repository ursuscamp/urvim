# Project Glossary

This document defines the key terminology used throughout the urvim codebase and specifications.

## Core Concepts

### Buffer
A text storage data structure backed by `imbl::Vector<Arc<str>>`. Each line is stored as an `Arc<str>` without trailing newline characters. Newlines exist implicitly between lines. The buffer supports efficient text manipulation with proper Unicode handling including grapheme clusters, combining characters, and emoji.

### Buffer ID
A newtype wrapper around `usize` that identifies a buffer stored in the global buffer pool. Buffer IDs are assigned monotonically starting at `0`.

**Related Terms:** Buffer, Buffer Pool, Window, Buffer View

### Buffer Pool
A process-global store that owns all live buffers and resolves them by `BufferId`. It deduplicates file-backed buffers by absolute path so that the same file is not loaded more than once.

**Related Terms:** Buffer, Buffer ID, Buffer View, Window

### Configuration
The resolved startup settings loaded from the command line and the user config file. Configuration is the single source of truth for user-facing startup options such as the active theme.

**Context:** CLI parsing, config file loading, startup initialization, global state

**Related Terms:** Configuration File, Theme, Globals

### Configuration File
The user-editable TOML file loaded from the XDG config directories at startup. It stores persistent configuration values that can be overridden by command-line flags.

**Context:** Startup configuration loading

**Related Terms:** Configuration, Theme

### Mutable Buffer Access
A pool-mediated mutation path that runs a closure while the buffer pool is still locked, so buffer edits cannot escape as detached snapshots or be committed later out of order.

**Context:** Buffer Pool mutation helpers and window edit flows

**Example:** `globals::with_buffer_mut(id, |buffer| buffer.insert_text(cursor, "x"))`

**Related Terms:** Buffer Pool, Buffer View, Window

### Cursor
A position in the buffer represented by `line` and `col` (byte position within line). The column can be from 0 to line byte length (inclusive, meaning cursor is at end of line).

### Cursor On a Character
In normal mode, the block cursor visually covers a character, indicating the cursor is positioned **before** that character. The notation "cursor on 'o'" means the cursor is positioned between the preceding character and 'o', i.e., "hell|o" represents cursor on 'o'. This is the Vim convention where the cursor selects the character beneath it.

### Cursor Sync
The process of normalizing a cursor position so it lands on a valid grapheme boundary before the editor stores or restores it. Cursor sync prevents persisted or recorded positions from pointing at invalid byte offsets after the buffer has changed.

**Context:** Cursor storage, jumplist restoration, buffer mutation, Unicode safety

**Related Terms:** Cursor, Grapheme Cluster, Jumplist, Buffer

### Action
An enum representing operations that the editor can perform in response to keypresses. Examples include `MoveLeft`, `MoveDown`, `InsertChar`, `SwitchToNormal`, etc.

### Advanced Glyphs
An optional set of editor configuration capabilities that request special glyph rendering behavior beyond plain text labels. The initial supported value is `nerdfont`.

**Context:** Configuration, tab bar rendering, status bar rendering

**Related Terms:** Filetype Glyph, Filetype, Tab Group, Status Bar, Configuration

### Auto-Indent
A user-facing insert-mode setting that selects how new lines should inherit indentation. The initial supported style is a neighbor mode that reuses nearby leading whitespace, and the setting is intentionally extensible so additional indentation strategies can be added later.

**Context:** Configuration, insert mode key handling, newline creation, buffer indentation inference

**Related Terms:** Insert Mode, Configuration, Buffer, Tab Behavior

### Change Operator
The `c` operator in operator-pending mode. It removes the resolved text range and then places the editor in insert mode when the operation succeeds. Examples include `cw`, `ciw`, `c$`, and `cG`.

**Related Terms:** Operator, Operator-Pending Mode, Operation Action, Delete Line, Change Line

### Comment Prefix
The syntax metadata value named `comment_prefix` that identifies the token inserted and removed by the line-comment toggle action. It is the canonical line-comment marker for a filetype and is used to determine whether a line should be commented or uncommented.

**Context:** Syntax metadata, line-comment toggle action, filetype-specific editing behavior

**Example:** `//` for C and Java, `#` for Python and shell-family syntaxes

**Related Terms:** Action, Filetype, Syntax Definition

### Repeat Record
A stored description of the last successful dot-repeatable edit. It records the originating normal-mode action, the count used for that edit, and any committed insert-mode text so `.` can replay the completed change.

**Related Terms:** Change Operator, Insert Mode, Action, Mode

### Mode
A trait that defines how the editor responds to key input in different states. Urvim implements two modes:
- **Normal Mode**: For navigation and command execution. Uses a steady block cursor.
- **Insert Mode**: For text input. Uses a steady bar cursor.

### Mode Kind
A lightweight editor-facing enum used to label the current mode in the status bar. It mirrors the live mode object managed by the main event loop and is returned by the `Mode::kind` method.

**Related Terms:** Mode, Status Bar, Layout

### Modified Buffer
A buffer state that indicates the in-memory contents differ from the last successful save on disk. Modified buffers are shown with a compact unsaved-changes marker in the tab bar and status bar.

**Context:** Buffer editing, save workflow, tab bar rendering, status bar rendering

**Related Terms:** Buffer, Tab Group, Status Bar, Save Command

### Save Command
The user-triggered save action, bound to `<C-s>`, that persists the active path-backed buffer to disk, refreshes filetype classification, and clears the modified state on success.

**Context:** Input handling, buffer persistence, modified-state tracking

**Related Terms:** Buffer, Modified Buffer, Filetype

### Theme
A resolved styling configuration that defines the editor's default style plus named UI and syntax styles. Themes determine how buffers, gutters, tab bars, and status bars inherit colors and text attributes during rendering.

**Context:** Theme loading, window rendering, status bar rendering, and future syntax highlighting

**Example:** `theme.default_style()` provides the base style that buffer content should inherit before any element-specific overlay is applied.

**Related Terms:** Default Style, Window, Gutter, Status Bar, Layout

### Tag
A hierarchical syntax label emitted by a grammar rule and consumed by a theme during style resolution. Tags are dot-separated and can become more specific as additional segments are added, such as `constant.integer` beneath `constant`.

**Context:** Syntax grammar definitions, syntax highlighting, theme style lookup

**Example:** A grammar rule may emit `constant.float`, while a theme styles `constant` broadly and overrides only `constant.float` for floating-point literals.

**Related Terms:** Syntax Highlighting, Syntax Definition, Theme

### Todo Marker
A literal, case-sensitive word token such as `TODO` or `FIXME` that appears inside a comment and receives marker-specific styling. Todo markers are matched only as standalone words, and each marker resolves to a theme tag under the `comment.*` hierarchy so the active theme can style it independently.

**Context:** Comment highlighting, syntax-aware rendering, theme syntax style lookup

**Example:** `TODO`, `FIXME`, `BUG`, and `NOTE`

**Related Terms:** Comment Prefix, Syntax Highlighting, Tag, Theme, Configuration

### Syntax Highlighting
A buffer-derived visual styling layer that classifies text spans by filetype-aware syntax categories such as comments, keywords, strings, numbers, and types. Syntax highlighting is expected to update when the underlying buffer text changes and to inherit colors from the active theme.

**Context:** Buffer rendering, filetype-aware styling, theme syntax styles

**Related Terms:** Buffer, Filetype, Theme, Default Style, Window, Syntax Definition

### Syntax Definition
A per-file TOML syntax configuration that describes how a filetype should be highlighted. Syntax definitions are loaded into memory and associated with one or more filetypes or language names. A syntax definition contains metadata plus one ordered `rules` list made of `regex` and `injection` rules.

**Context:** Syntax config loading, highlight resolution, filetype lookup

**Related Terms:** Syntax Highlighting, Regex Rule, Injection Rule, Filetype, Configuration File

### Syntax Registry
The in-memory catalog of builtin syntax definitions and metadata used to resolve syntax names, aliases, filename matches, and first-line matches. In urvim, the registry can keep a syntax in raw form until it is promoted for actual tokenization.

**Context:** Syntax loading, filetype detection, nested syntax resolution

**Related Terms:** Syntax Definition, Syntax Promotion, Filetype, Injected Syntax

### Syntax Promotion
The on-demand compilation of a raw syntax definition into its runtime form the first time that syntax is needed. Promotion preserves the same highlighting behavior while avoiding eager compilation of unused syntaxes.

**Context:** Syntax registry resolution, tokenizer-driven injected syntax lookup

**Related Terms:** Syntax Registry, Syntax Definition, Syntax Highlighting, Injected Syntax

### String Interpolation
A nested syntax inside a string literal that highlights embedded expressions or language fragments using the host language grammar or another declared injected syntax. String interpolation is used when the text inside a string should be tokenized as code instead of plain string content.

**Context:** String literal highlighting, injected syntax bodies, host-language grammar definitions

**Related Terms:** Injection Rule, Injected Syntax, Escape Sequence, Syntax Highlighting

### Injected Syntax
A nested syntax that is resolved from the text inside a surrounding syntax rule, such as a Markdown fenced code block that selects a language by its fence tag. Injected syntax reuses the same syntax registry and style categories as top-level syntax definitions.

**Context:** Injected highlighting, Markdown fences, nested highlighting

**Related Terms:** Injection Rule, Syntax Definition, Syntax Highlighting

### Jumplist
A per-window navigation history that stores meaningful cursor locations so the editor can jump backward and forward through recent positions. A jumplist entry identifies both the buffer and the cursor position within that buffer.

**Context:** Window navigation, cursor history, jump actions

**Related Terms:** Window, Buffer, Cursor

### Syntax Alias
A secondary language name declared in syntax metadata that resolves to the same syntax definition as the canonical syntax name. Syntax alias labels are used for injected language names such as Markdown fence tags and other user-provided selectors.

**Context:** Syntax metadata, injected syntax resolution, label lookup

**Related Terms:** Syntax Definition, Injected Syntax, Filetype

### Regex Rule
A syntax rule that matches the current input with a regular expression, applies a tag, and can update context by pushing or popping markers or payload-bearing entries.

**Context:** Syntax grammar definitions, syntax highlighting, context-driven tokenization

**Related Terms:** Syntax Definition, Injection Rule, Syntax Highlighting, Context Entry

### Injection Rule
A syntax rule that delegates highlighting of the current body to another syntax when the active context permits it. Injection rules are how embedded languages such as HTML script bodies, Markdown fences, and template bodies choose a nested syntax.

**Context:** Injected highlighting, nested bodies, syntax registry lookup

**Related Terms:** Syntax Definition, Regex Rule, Injected Syntax, Context Entry

### Context Entry
An item stored on the active context stack. A context entry may be a plain marker or a payload-bearing value that carries opener-specific text forward so later rules can require a matching prefix or exact payload.

**Context:** Context-driven tokenization, opener/closer matching, heredoc-style bodies

**Related Terms:** Regex Rule, Injection Rule, Syntax Highlighting

### Default Style
The base `Style` supplied by a theme before any UI- or syntax-specific overlay is applied. Unspecified style fields in a rendered region should inherit from this style.

**Context:** Theme resolution and renderer base-style application

**Example:** A buffer line that only sets foreground color should still inherit the theme default background color.

**Related Terms:** Theme, Window, Screen, Cell

### Escape Sequence
A grammar-defined literal sequence inside a string that represents a special character or encoded value, such as `\n` or `\t`. Escape sequences are highlighted as part of the host string grammar and do not require a separate syntax definition.

**Context:** String literal highlighting, regex-driven tokenization

**Related Terms:** String Interpolation, Regex Rule, Syntax Highlighting

### Filetype
A syntax-defined buffer classification that provides a canonical `name`, a user-facing `display_name`, filename-matching regexes, and first-line-matching regexes. Filetypes are derived from syntax grammar metadata and are used for highlighting, lookup, and user-facing labels such as the status bar.

**Context:** Buffer metadata, syntax loading, status bar rendering

**Related Terms:** Buffer, Status Bar, Syntax Definition

### Filetype Glyph
A syntax metadata value that stores the icon used to represent a filetype in compact UI surfaces such as the tab bar and status bar.

**Context:** Syntax metadata, tab bar rendering, status bar rendering

**Related Terms:** Advanced Glyphs, Filetype, Tab Group, Status Bar

### Filetype Glyph Color
The default foreground color associated with a filetype glyph when the editor renders that icon.

**Context:** Syntax metadata, tab bar rendering, status bar rendering

**Related Terms:** Filetype Glyph, Theme, Tab Group, Status Bar

### Window
A rendering component that owns a Buffer View and displays its buffer on screen. It handles cursor positioning, scrolling, and text rendering with gutter.

**Related Terms:** Tab Group, Buffer, Screen

### Screen
A double-buffered terminal renderer. Maintains current and previous frame buffers for diff-based rendering - only writes changed cells to the terminal.

### Status Bar
A one-line footer rendered by the root layout that shows editor metadata such as the active mode, active buffer name, cursor position, and file progress.

**Related Terms:** Layout, Mode, Buffer, Cursor

### Gutter
The left margin area that displays line numbers. Shows a distinct background color to separate it from content.

### Tab Group
A container that owns multiple windows, displays a horizontal tab bar, and routes editing actions to the active window. The tab bar can scroll horizontally when more tabs exist than fit in the visible terminal width.

**Related Terms:** Layout, Window, Screen, Buffer

### Keymap
A data structure that maps key sequences to actions. Supports multi-key bindings like `dd` (delete line) or `gg` (go to first line). Implementations include Trie-based keymaps for fixed sequences, character scan keymaps for parameter-based motions, and chained keymaps that combine multiple keymaps.

**Related Terms:** Canonical String, Key String Parser, TrieKeymap, Character Scan Keymap, Chained Keymap

### Character Scan Keymap
A stateless keymap that matches two-key sequences for character scan motions (f, F, t, T). The first key is the trigger (f/F/t/T) and the second key is the target character. Returns the corresponding action with the character as a parameter.

### Chained Keymap
A keymap wrapper that delegates `get_action` and `is_prefix` calls to multiple sub-keymaps in sequence, trying each until one returns a non-None result. Used to combine trie-based keymaps with character scan keymaps.

### Layout
A container widget that owns one or more higher-level UI regions, positions them on screen, and sizes them relative to the available terminal space. The first layout implementation owns a single tab group plus a footer status bar and serves as the root container above `Tab Group`.

**Related Terms:** Tab Group, Window, Screen, Status Bar

## Text Navigation

### Boundary
Types of word boundaries for text navigation:
- **Word**: Alphanumeric characters + underscore
- **WordEnd**: End of a word
- **BigWord**: Any non-whitespace character
- **BigWordEnd**: End of a BigWord

### BigWord Text Object
An operator-pending text object family that selects whitespace-delimited runs using `Boundary::BigWord` semantics. In urvim, the expected bindings are `iW` for the inner selection and `aW` for the around selection.

**Context:** Operator-pending mode, text object resolution, normal-mode keymaps

**Related Terms:** Boundary, Text Object, Operator-Pending Mode

### Grapheme Cluster
A user-perceived character, which may consist of multiple Unicode code points (e.g., emoji with skin tone modifiers, combining characters).

### Visual Column
The display width of text in the terminal, accounting for wide characters (CJK, emoji) that occupy 2 terminal cells.

## Input Handling

### Key
A complete key event combining a key code with optional modifiers (Shift, Alt, Ctrl, Super, Hyper, Meta).

### KeyCode
The type of key pressed, including:
- Character keys (`Char(char)`)
- Special keys (`Enter`, `Backspace`, `Delete`, `Tab`, `Esc`, etc.)
- Navigation keys (`Up`, `Down`, `Left`, `Right`, `Home`, `End`, etc.)
- Function keys (`F1` through `F12`)

### Modifiers
Keyboard modifier state flags that can be combined:
- `SHIFT` - Shift key
- `ALT` - Alt/Option key
- `CTRL` - Control key
- `SUPER` - Super/Win/Cmd key
- `HYPER` - Hyper key
- `META` - Meta key

### Canonical String
The normalized string representation of a key, used for keymap lookup. Examples:
- `h`, `j`, `k`, `l` for movement keys
- `<Enter>`, `<Esc>`, `<Backspace>` for special keys
- `<C-q>` for Ctrl+q
- `<Space>` for the space bar

**Related Terms:** Keymap, Key String Parser

### Event
Terminal input events:
- `Key(Key)` - A key press
- `Resize(rows, cols)` - Terminal size change
- `Paste(text)` - Bracketed paste content

### Insert Mode Escape Binding
A user-configurable canonical key string that exits insert mode and switches back to normal mode. It is added alongside the built-in `<Esc>` binding so users can choose a more ergonomic home-row escape sequence such as `jk`.

**Context:** Configuration loading, insert mode keymap setup

**Example:** `insert_escape = "jk"` adds `j` followed by `k` as an alternate way to leave insert mode.

**Related Terms:** Insert Mode, Keymap, Canonical String, Configuration

### Tab Behavior
The user-facing insert-mode tab handling setting that chooses between simple and smart behavior. Simple behavior always uses the configured tab insertion setting, while smart behavior infers indentation style from existing buffer contents and falls back to the configured insertion setting when no clear style exists.

**Context:** Configuration, insert mode key handling, buffer indentation inference

**Related Terms:** Tab Insertion, Insert Mode, Configuration, Buffer

### Tab Insertion
The user-facing setting that chooses whether insert-mode `Tab` inserts literal tab characters or space-based indentation. It is the concrete insertion style used by simple tab behavior and by smart tab behavior when inference is unavailable.

**Context:** Configuration, insert mode key handling

**Related Terms:** Tab Behavior, Insert Mode, Configuration

### Key String Parser
A helper that turns a canonical key string into an ordered list of trie key tokens. Every non-`<...>` character is treated as one key, and every bracketed canonical special key is treated as one token.

**Context:** Trie keymap insertion helpers and editor key binding setup

**Example:** `gg` becomes `["g", "g"]`, `<C-s>` becomes `["<C-s>"]`, and `d<LessThan>` becomes `["d", "<LessThan>"]`

**Related Terms:** Canonical String, Keymap

## Rendering

### Cell
A single character cell in the screen grid, containing:
- `style`: Text styling (foreground/background colors)
- `text`: The grapheme cluster content

### Position
A 2D coordinate `(row, col)` representing a position on screen.

### Size
A 2D dimension `(rows, cols)` representing the size of a region.

### Viewport
The visible portion of the buffer being displayed. Defined by scroll offset and visible area size.

### Scroll Offset
The position in the buffer that corresponds to the top-left of the viewport.

## Actions (Common)

### Movement Actions
- `MoveLeft`, `MoveRight`, `MoveUp`, `MoveDown` - Basic cursor movement
- `ForwardTo(Boundary)` - Move to next word boundary (w, e)
- `BackTo(Boundary)` - Move to previous word boundary (b)
- `MoveToLineStart` - Move to column 0 (0)
- `MoveToLineContentStart` - Move to first non-whitespace (^)
- `MoveToLineEnd` - Move to end of line ($)
- `MoveToFirstLine` - Go to first line (gg)
- `MoveToLastLine` - Go to last line (G)
- `MoveToScreenTop/Middle/Bottom` - H/M/L viewport navigation
- `FindForward(char)` - Move to next occurrence of char (f)
- `FindBackward(char)` - Move to previous occurrence of char (F)
- `TillForward(char)` - Move to position before next occurrence (t)
- `TillBackward(char)` - Move to position after previous occurrence (T)

### Edit Actions
- `InsertChar(c)` - Insert a character
- `DeleteBackward` - Delete character before cursor (backspace)
- `DeleteForward` - Delete character at cursor (x)
- `DeleteLine` - Delete current line (dd)
- `ChangeLine` - Delete line and enter insert mode (cc)
- `JoinWithSpace` - Join lines with space (J)
- `JoinWithoutSpace` - Join lines without space (gJ)
- `OpenLineBelow` - Create line below (o)
- `OpenLineAbove` - Create line above (O)

### Mode Actions
- `SwitchToNormal` - Enter Normal mode
- `SwitchToInsert` - Enter Insert mode
- `AppendAfterCursor` - Move right and enter Insert mode (a)
- `AppendToLineEnd` - Move to line end and enter Insert mode (A)
- `InsertAtLineStart` - Move to first non-whitespace and enter Insert mode (I)

### Count Actions
- `Count(usize, Box<Action>)` - Repeat an action N times (e.g., `5j` moves down 5 lines)

### Operator
An action that waits for a motion or text object to define its target region. Examples: `Delete`, `Change`, `Yank`.

### Text Object
A selection of text defined by boundaries (start and end positions). Text objects are used with operators in operator-pending mode. Examples: `InnerWord`, `AroundWord`, `InnerBracket`, `AroundBracket`. Word-family variants can be based on either `Boundary::Word` or `Boundary::BigWord` semantics.

### Operator-Pending Mode
A state where the editor waits for a motion or text object after an operator key is pressed. For example, after pressing `d`, the editor waits for `w` (motion) or `iw` (text object) to define what to delete.

### Operation Action
`Action::Operation(Operator, TextObject)` - A compositional action combining an operator with a text object. Instead of many individual action variants, operations use this structure for extensibility.

### Count Prefix
A numeric prefix that multiplies with the action. urvim supports two count placements:
- **Leading count**: Before the operator (e.g., `3diw` = delete 3 inner words)
- **Sub-count**: After the operator (e.g., `d3iw` = delete 3 inner words)
- **Combined**: Multiplicative (e.g., `3d3iw` = 3 × 3 = 9 inner words)

### Inner Word (Text Object)
A word selected without surrounding whitespace boundaries. If cursor is inside whitespace, selects the whitespace region. If cursor is inside a word, selects that word.

### Around Word (Text Object)
A word selected with trailing whitespace included. If cursor is inside whitespace, selects whitespace plus the trailing word. If cursor is inside a word, selects that word plus all trailing whitespace.

### Bracket Text Object
A text object that selects text inside or around a matching delimiter pair such as parentheses, square brackets, curly braces, or angle brackets. Bracket text objects are used with operators in operator-pending mode and include inner and around forms. Related terms: Inner Bracket Text Object, Around Bracket Text Object, Text Object.

### Inner Bracket Text Object
A bracket text object that selects only the text between matching delimiters and excludes the delimiters themselves. It follows Vim-compatible bracket-object matching rules for the supported delimiter families. Related terms: Bracket Text Object, Around Bracket Text Object.

### Around Bracket Text Object
A bracket text object that selects the matching delimiters together with the enclosed text. It follows Vim-compatible bracket-object matching rules for the supported delimiter families. Related terms: Bracket Text Object, Inner Bracket Text Object.

### Quote Text Object
A text object that selects text inside or around matching quote delimiters such as single quote, double quote, or backtick. Quote text objects are used with operators in operator-pending mode and include inner and around forms. Related terms: Inner Quote Text Object, Around Quote Text Object, Text Object.

### Inner Quote Text Object
A quote text object that selects only the text between matching quote delimiters and excludes the delimiters themselves. It follows Vim-compatible quote-object matching rules for the supported quote delimiters. Related terms: Quote Text Object, Around Quote Text Object.

### Around Quote Text Object
A quote text object that selects the matching quote delimiters together with the enclosed text. It follows Vim-compatible quote-object matching rules for the supported quote delimiters. Related terms: Quote Text Object, Inner Quote Text Object.

## Spec-Related Terms

### Requirements Document
A specification file describing the feature's purpose, user stories, functional requirements, and acceptance criteria.

### Design Document
A technical specification describing the implementation approach, data structures, algorithms, and interfaces.

### Tasks Document
A checklist of implementation steps derived from the requirements and design documents.

### Bug Report
A specification document describing a bug, including steps to reproduce, expected behavior, and actual behavior.
