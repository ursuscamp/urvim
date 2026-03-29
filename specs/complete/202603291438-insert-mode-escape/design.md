# Insert Mode Escape Binding - Technical Design
## Architecture Overview
This feature adds an optional insert-mode escape binding to startup configuration and threads that value into insert-mode keymap construction.

The implementation keeps the current hard-coded `<Esc>` behavior intact and layers one additional binding on top of the existing insert-mode `TrieKeymap`. That means:

- `<Esc>` continues to exit insert mode unconditionally.
- A configured alternate escape binding is treated as another exact key sequence that maps to `Action::SwitchToNormal`.
- Insert mode still uses the same key-sequence matching rules as the rest of the editor.

To keep configuration failures predictable, the key string used for the alternate escape binding should be validated during config loading rather than deferred until the user first enters insert mode.

## Interface Design
### Configuration schema
Add one optional TOML field to the startup config schema:

```toml
theme = "Friday Night"
insert_escape = "jk"
```

Recommended Rust shape:

- `PartialConfig.insert_escape: Option<String>`
- `Config.insert_escape: Option<String>`

The field stores a canonical key string, not a raw terminal escape code. It should accept both single-key and multi-key sequences that can be represented by the existing key parser.

### Insert mode construction
Keep `InsertMode::new() -> Self` as the public constructor. During construction, it should read the resolved config through the existing global config access and register the alternate escape binding if one is present.

This keeps the app-facing API unchanged and avoids threading the config through every insert-mode call site.

### Key string validation
Introduce a fallible validation path for canonical key strings so config loading can fail cleanly on invalid values.

Recommended shape:

- `TrieKeymap::try_insert_str(keys: &str, action: Action) -> Result<(), KeyStringParseError>`
- or a shared `validate_key_string(keys: &str) -> Result<Vec<String>, KeyStringParseError>`

The validation helper should be reused by both config loading and insert-mode keymap setup so the same canonical-string rules apply in both places.

## Data Models
### `Config`
Add a new optional field:

- `insert_escape: Option<String>`

Semantics:

- `None` means no alternate escape binding is configured.
- `Some(value)` means the value should be treated as a canonical key string.

### `PartialConfig`
Add the same optional field with `serde(deny_unknown_fields)` still enabled, so unknown config keys remain errors.

### Validation rules
- Empty strings are invalid.
- Whitespace-only strings are invalid.
- Malformed canonical key strings are invalid.
- Valid multi-key sequences are allowed as long as the parser can tokenize them.

## Key Components
### Config loader
The config module remains the single place where startup config is resolved and validated. It should:

- Deserialize the new `insert_escape` field.
- Validate the field if present.
- Preserve the existing theme resolution behavior.
- Continue surfacing parse and I/O errors through `ConfigLoadError`.

### Insert mode
`InsertMode` remains responsible for translating keys into actions while the editor is in insert mode.

Its keymap setup should:

- Register `<Esc>` as `Action::SwitchToNormal` unconditionally.
- Register the configured alternate escape binding if one is present.
- Keep all existing insert-mode bindings unchanged.

### Key parsing / keymap helpers
The canonical key string parser already splits strings such as `gg` or `<C-s>` into token sequences. The design relies on that behavior and adds a fallible validation entry point so invalid config cannot panic at startup.

## User Interaction
### Example config
```toml
insert_escape = "jk"
```

### Runtime behavior
- Pressing `<Esc>` in insert mode exits immediately.
- Pressing the configured alternate escape binding in insert mode exits immediately.
- While the user is typing the alternate sequence, insert mode waits for the full binding to resolve before deciding whether it is an escape sequence or normal text input.
- The custom binding has no effect in normal mode.

### Sequence examples
- `insert_escape = "jk"`: pressing `j` then `k` exits insert mode.
- `insert_escape = "<C-[>"`: pressing Ctrl+[ exits insert mode, if the canonical parser accepts the sequence.

## External Dependencies
No new external crates are required. This feature uses existing serde/toml config parsing and the current editor keymap infrastructure.

## Error Handling
- If the config value is missing, the editor falls back to the built-in `<Esc>` behavior only.
- If the config value is empty or whitespace-only, config loading fails with a clear validation error.
- If the config value cannot be tokenized as a canonical key sequence, config loading fails with a clear validation error.
- If the user types a partial multi-key sequence in insert mode, the mode waits for more input rather than exiting early.
- If a partial sequence later proves invalid, it should be treated as invalid input and cleared from the insert-mode buffer.

## Security
This feature does not introduce new security-sensitive behavior. It only changes how local keyboard input is mapped to existing editor actions.

## Configuration
Update the user-facing config docs to describe:

- the new `insert_escape` option
- its canonical-string format
- the fact that it augments, rather than replaces, `<Esc>`
- invalid values are rejected at startup

No new CLI flag is required in this design.

## Component Interactions
1. The config loader reads `config.toml` and populates `Config.insert_escape`.
2. During startup, the app stores the resolved config globally as it already does today.
3. When the editor enters insert mode, it constructs `InsertMode` with `InsertMode::new()`.
4. `InsertMode::new()` reads the resolved config from the global config store and registers the custom binding in its trie keymap if one is configured.
5. The event loop continues dispatching key input through the active mode as before.

This keeps the change localized to config resolution, insert-mode setup, and documentation.

## Platform Considerations
- The feature should work consistently across terminals and operating systems because it relies on urvim's canonical key abstraction, not platform-specific scan codes.
- Multi-key escape sequences may be affected by terminal repeat rate or IME behavior in the same way as other key sequences, but they should follow the existing insert-mode buffering rules.
