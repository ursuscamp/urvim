# Project Glossary

This document defines the key terminology used throughout the urvim codebase and specifications.

## Core Concepts

### Buffer
A text storage data structure backed by `imbl::Vector<Arc<str>>`. Each line is stored as an `Arc<str>` without trailing newline characters. Newlines exist implicitly between lines. The buffer supports efficient text manipulation with proper Unicode handling including grapheme clusters, combining characters, and emoji.

### Cursor
A position in the buffer represented by `line` and `col` (byte position within line). The column can be from 0 to line byte length (inclusive, meaning cursor is at end of line).

### Action
An enum representing operations that the editor can perform in response to keypresses. Examples include `MoveLeft`, `MoveDown`, `InsertChar`, `SwitchToNormal`, etc.

### Mode
A trait that defines how the editor responds to key input in different states. Urvim implements two modes:
- **Normal Mode**: For navigation and command execution. Uses a steady block cursor.
- **Insert Mode**: For text input. Uses a steady bar cursor.

### Window
A rendering component that owns a Buffer and displays it on screen. It handles cursor positioning, scrolling, and text rendering with gutter.

### Screen
A double-buffered terminal renderer. Maintains current and previous frame buffers for diff-based rendering - only writes changed cells to the terminal.

### Gutter
The left margin area that displays line numbers. Shows a distinct background color to separate it from content.

### Keymap
A data structure (implemented as a Trie) that maps key sequences to actions. Supports multi-key bindings like `dd` (delete line) or `gg` (go to first line).

## Text Navigation

### Boundary
Types of word boundaries for text navigation:
- **Word**: Alphanumeric characters + underscore
- **WordEnd**: End of a word
- **BigWord**: Any non-whitespace character
- **BigWordEnd**: End of a BigWord

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

### Event
Terminal input events:
- `Key(Key)` - A key press
- `Resize(rows, cols)` - Terminal size change
- `Paste(text)` - Bracketed paste content

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

## Spec-Related Terms

### Requirements Document
A specification file describing the feature's purpose, user stories, functional requirements, and acceptance criteria.

### Design Document
A technical specification describing the implementation approach, data structures, algorithms, and interfaces.

### Tasks Document
A checklist of implementation steps derived from the requirements and design documents.

### Bug Report
A specification document describing a bug, including steps to reproduce, expected behavior, and actual behavior.
