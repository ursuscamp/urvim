# Filetype Detection - Technical Design

## Architecture Overview

urvim will add a small filetype classification layer to the buffer domain and thread the resolved filetype through the existing layout and status bar rendering path. The buffer remains the source of truth for its own filetype, and the status bar only consumes a display label derived from that buffer state.

The design keeps detection intentionally simple:

- filename-based matches are checked first
- if the filename does not determine a filetype, the first line is inspected for a shebang
- if neither source is conclusive, the buffer falls back to a default general-purpose filetype

This keeps detection deterministic and avoids pushing filetype logic into the renderer.

### Flow

```text
Buffer load or relevant text change
  -> resolve filetype from filename
  -> if inconclusive, inspect shebang in first line
  -> fall back to plain text
  -> store resolved filetype on the buffer
  -> layout reads filetype from active buffer view
  -> status bar renders filetype label
```

## Interface Design

### Filetype enum

Add a public enum in the buffer domain that represents the supported common filetypes.

Representative shape:

```rust
pub enum Filetype {
    PlainText,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Lua,
    Shell,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Perl,
    Haskell,
    Elixir,
    Erlang,
    OCaml,
    FSharp,
    Kotlin,
    Scala,
    Swift,
    Dart,
    Zig,
    Nim,
    Julia,
    R,
    Markdown,
    Json,
    Toml,
    Yaml,
    Html,
    Css,
}
```

The enum should expose a human-readable label for display purposes.

### Buffer API

The buffer should expose the resolved filetype through a read-only method.

Representative shape:

```rust
impl Buffer {
    pub fn filetype(&self) -> Filetype;
}
```

If the buffer implementation stores the resolved filetype eagerly, it should also provide an internal refresh path used after load, path changes, or content changes that affect the first line.

### Status bar context

Extend `StatusBarContext` to include a filetype label.

Representative shape:

```rust
pub struct StatusBarContext<'a> {
    pub mode_label: &'a str,
    pub filetype_label: &'a str,
    pub buffer_name: &'a str,
    pub cursor_line: usize,
    pub cursor_byte_col: usize,
    pub line_count: usize,
}
```

### Status bar text

Update the footer text format to include the filetype in a stable position near the other buffer metadata.

Example shape:

```text
NORMAL | Rust | notes.rs | 3:7b | 22%
```

## Data Models

### Filetype classification

The supported filetype set should include a curated list of common code-editor filetypes, with a general-purpose fallback for buffers that do not match a more specific entry.

#### Filename matches

Filename matching should support both:

- extension-based rules such as `.rs`, `.py`, `.js`, `.ts`, `.lua`, `.go`, `.rb`, `.php`, `.pl`, `.hs`, `.ex`, `.erl`, `.ml`, `.fs`, `.kt`, `.scala`, `.swift`, `.dart`, `.zig`, `.nim`, `.jl`, `.r`, `.md`, `.toml`, `.json`, `.yaml`, `.yml`, `.html`, and `.css`
- common extensionless names such as `Makefile` or `Dockerfile`

#### Shebang matches

Shebang detection should inspect only the first line and should treat the interpreter token as the source of truth after stripping:

- the leading `#!`
- any `/usr/bin/env` wrapper
- any trailing interpreter arguments

Examples:

- `#!/usr/bin/env python3 -O` -> `Python`
- `#!/bin/bash` -> `Shell`
- `#!/usr/bin/env node` -> `JavaScript`
- `#!/usr/bin/env ruby` -> `Ruby`
- `#!/usr/bin/env perl` -> `Perl`
- `#!/usr/bin/env elixir` -> `Elixir`
- `#!/usr/bin/env scala` -> `Scala`

## Key Components

### Filetype detector

Responsibilities:

- resolve filetype from filename
- fall back to shebang detection when filename-based resolution is not enough
- return the fallback filetype when no match is found

Dependencies:

- buffer filename
- buffer first line contents

### Buffer integration

Responsibilities:

- store the resolved filetype with the buffer
- refresh the filetype when the buffer's relevant inputs change
- expose the filetype through a public getter

Dependencies:

- filetype detector
- buffer loading and mutation code

### Layout and status bar integration

Responsibilities:

- read the active buffer filetype from the active buffer view
- convert the filetype to a display label
- render the label in the footer without affecting existing metadata placement beyond the added field

Dependencies:

- `BufferView`
- `StatusBar`
- `Layout`

## User Interaction

The only user-visible change is the footer now shows a filetype label for the active buffer. No new input flows are introduced.

## External Dependencies

No new external dependencies are required. The implementation can stay within the existing buffer, window, and status bar modules.

## Error Handling

Detection should never fail the editor startup or rendering flow.

- If filename classification does not match anything, use the fallback filetype.
- If the buffer has fewer than one line, treat shebang detection as inconclusive and use the fallback filetype.
- If the first line is not a valid shebang, ignore it and use the fallback filetype unless the filename already matched.

The buffer should never report an error to the user for unknown filetypes.

## Security

Filetype detection must be read-only and must not execute shebang targets, open processes, or inspect anything beyond the buffer's own metadata and first line. This keeps classification safe for arbitrary file contents.

## Configuration

No user-facing configuration is required for the first version of filetype detection.

## Component Interactions

1. Buffer loading creates or refreshes the buffer's filetype classification.
2. A buffer view exposes the active buffer's metadata to the layout.
3. Layout passes the filetype label into the status bar context.
4. The status bar renders the label alongside mode, buffer name, cursor position, and progress.

## Platform Considerations

Filename matching should be normalized enough to handle platform-specific path conventions and case differences for extensionless names where applicable. Shebang parsing should be path- and shell-agnostic because it only examines the first line text.
