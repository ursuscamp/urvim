# Command Line for Save and Edit - Technical Design

## Architecture Overview
This feature introduces a command-line overlay pipeline in the UI layer and a reusable floating window presentation primitive.

High-level flow:
1. Normal mode receives `:` and emits a command-line open intent.
2. Root UI switches focus to the command-line overlay.
3. Command-line widget captures keys, edits its input buffer, and renders in a centered bordered floating window.
4. On submit, command text is parsed into a command request.
5. Command dispatcher executes `save`/`edit` against editor/buffer services.
6. Success/failure results are emitted as UI commands (primarily notifications for errors).
7. Command-line overlay closes after the execute attempt.

In parallel, the notification banner moves from its dedicated floating drawing logic to the shared floating window abstraction so both overlays share layout and border behavior.

## Interface Design
### Command-line lifecycle interface
- `open_command_line()`
  - Behavior: activates command-line overlay with empty or history-selected input state.
- `close_command_line()`
  - Behavior: clears active command-line UI focus.
- `submit_command_line(input: &str)`
  - Behavior: parses and executes command text; enqueues notification commands on failures.

### Command parsing interface
- `parse_command(input: &str) -> Result<ParsedCommand, CommandParseError>`
  - Supports shell-like quoted tokens for path arguments.
  - Rejects unknown command names and invalid argument counts.

Parsed command shape:
- `ParsedCommand::Save { path: Option<String> }`
- `ParsedCommand::Edit { path: Option<String> }`

### Command execution interface
- `execute_command(cmd: ParsedCommand, ctx: &mut EditorContext) -> Result<(), CommandExecError>`
  - `Save { path: None }`: requires active buffer path.
  - `Save { path: Some }`: requires non-existing target path.
  - `Edit { path: None }`: creates unnamed buffer and switches focus.
  - `Edit { path: Some }`: reuses existing open buffer by canonical path or opens new one.

### Floating window interface
A shared UI primitive for bordered floating overlays:
- constructor with anchor policy + size policy + border style.
- render contract: resolves rectangle from screen size and draws border + content region.
- used by:
  - notification banner (top-right anchor)
  - command line (center anchor)

## Data Models
### `ParsedCommand`
- Enum with variants:
  - `Save { path: Option<String> }`
  - `Edit { path: Option<String> }`
- Constraints:
  - Command names are case-sensitive (`save`, `edit`).
  - `save`/`edit` accept at most one optional path argument.

### `CommandLineState`
- Fields:
  - `input: String`
  - `cursor_col: usize`
  - `history: Vec<String>`
  - `history_index: Option<usize>` (navigation state)
- Constraints:
  - History is process-local and reset on editor restart.
  - Entered command is appended to history on submit attempts.

### `FloatingWindowSpec`
- Fields (conceptual):
  - `anchor` (e.g., `Center`, `TopRight`)
  - `size` (fixed/min-constrained content sizing)
  - `border` (style and glyph set)
  - `padding` (optional)
- Constraints:
  - Must safely clamp to viewport bounds.
  - Border rendering must preserve current theme highlight behavior.

## Key Components
### 1) Floating window module (new/generic)
Responsibilities:
- Resolve floating rectangle from viewport and anchor policy.
- Render bordered container consistently.
- Expose content area to caller renderers.

Public API (shallow):
- build floating spec
- compute frame rect
- draw frame + delegate content draw

Dependencies:
- screen renderer
- theme/UI highlight lookup
- existing border glyph selection logic

### 2) Notification banner renderer (refactor)
Responsibilities:
- Keep existing notification queue/timing behavior.
- Delegate floating frame geometry + border draw to floating window module.

Public behavior unchanged:
- same placement semantics and message wrapping output.

Dependencies:
- notification queue state
- floating window module

### 3) Command-line overlay widget (new)
Responsibilities:
- Own command-line input and history navigation state.
- Handle key events while active.
- Render prompt/input inside shared floating frame.
- Emit command submission intent.

Dependencies:
- floating window module
- key event canonicalization
- root intent dispatcher

### 4) Command parser + dispatcher (new)
Responsibilities:
- Parse user input into `ParsedCommand` with quoted argument support.
- Validate command name and argument shape.
- Execute against active editor context.
- Surface failures via notification commands.

Dependencies:
- buffer pool and active window services
- file save/open services
- notification command enqueue path

## User Interaction
### Open and cancel
- User presses `:` in normal mode.
- Overlay appears centered, bordered, focused for input.
- `Esc` closes overlay without execution.

### Edit command line
- Printable characters insert at cursor.
- `Backspace` removes character before cursor.
- History recall:
  - `Up` / `Ctrl-p`: previous command
  - `Down` / `Ctrl-n`: next command

### Execute
- `Enter` submits current input.
- Parser resolves command + optional quoted path argument.
- Execution runs and command line closes regardless of result.
- Errors appear in notification banner.

## External Dependencies
- No new external crates are required.
- Uses existing filesystem/path operations already present in the editor.
- Uses existing notification queue/dispatch system.

## Error Handling
### Parse-time
- Empty input: treat as no-op execute and close.
- Unknown command: `Unknown command: <name>` notification.
- Invalid arity: command-specific usage error notification.
- Invalid quoting: parse error notification.

### Execute-time
- `save` with unnamed buffer: error notification.
- `save <path>` where target exists: error notification, no overwrite.
- `edit <path>` with unreadable path/open failure: error notification.
- All failures are non-fatal and return control to normal editing.

Recovery strategy:
- Preserve editor state; do not mutate buffer/window selection on failed operations.
- Always close command line after attempted execution.

## Security
- Input is treated as editor commands only; no shell execution.
- Path arguments are handled as plain filesystem paths, not interpolated command strings.
- Overwrite protection for `save <path>` reduces accidental destructive writes.

## Configuration
- No new user configuration options in initial implementation.
- Existing keymap binding for `:` in normal mode is required/added in code defaults.
- Existing theme/border styling applies through shared floating window primitive.

## Component Interactions
1. Normal mode key handler emits open-command-line command.
2. Root dispatcher enables command-line overlay and routes UI key events to it.
3. Command-line widget updates local state or emits submit intent.
4. Parser returns `ParsedCommand` or parse error.
5. Dispatcher executes command using editor/buffer services.
6. Dispatcher emits notifications for failures.
7. Overlay is closed.
8. Notification banner renders via floating window abstraction.

## Platform Considerations
- Terminal key differences:
  - Ensure `Ctrl-p` and `Ctrl-n` are recognized consistently by key canonicalization.
- Path handling:
  - Respect existing path normalization/canonicalization logic for open-buffer dedupe.
  - Keep behavior consistent on macOS/Linux path semantics.
- Rendering constraints:
  - Floating frame geometry must clamp in small terminals so borders/content remain visible.