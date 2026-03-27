# Unified Config Framework - Technical Design

## Architecture Overview

urvim will gain a single startup configuration pipeline that reads a TOML config file from the XDG config directories, merges that file with command-line overrides, and stores the resolved result in global state before the editor begins rendering. The first config value in the file schema is the theme, and the initial implementation uses that field as the canonical startup setting.

The design separates the startup data into two layers:

- `Config` stores the resolved startup configuration after file and CLI values are merged.
- `Theme` remains the resolved rendering object derived from the config's theme value.

This keeps startup configuration independent from rendering data while still giving the rest of the editor a single source of truth for user-facing startup options.

### Flow

```text
Startup
  -> parse CLI arguments
  -> locate and parse optional TOML config file from XDG config directories
  -> merge file values with CLI overrides
  -> load built-in themes
  -> validate the resolved theme name against the registry
  -> store resolved Config in globals
  -> resolve the active Theme from Config.theme
  -> store active Theme in globals
  -> enter the main editor loop
```

## Interface Design

### CLI

`src/main.rs` keeps the existing `--theme` flag, but the flag becomes an override input to the configuration merge step instead of being the only theme source.

```rust
#[derive(Parser)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    files: Vec<std::path::PathBuf>,
}
```

Behavior:

- if `--theme` is omitted, the config loader may supply a theme from the TOML file
- if the TOML file omits `theme`, startup falls back to the existing default theme behavior
- if both the file and CLI provide a theme, the CLI value wins

### Config module

Add a dedicated `config` module that owns the schema, parsing, and merge logic.

Proposed public types:

```rust
pub struct Config {
    pub theme: String,
}

pub struct PartialConfig {
    pub theme: Option<String>,
}

pub struct ConfigSource {
    pub path: Option<std::path::PathBuf>,
    pub config: PartialConfig,
}

pub enum ConfigLoadError { /* io, parse, validation */ }

impl Config {
    pub fn resolve(file: Option<PartialConfig>, cli_theme: Option<String>) -> Self;
}
```

Responsibilities:

- load the optional TOML file from the XDG config search path
- parse the file into a partial config that reflects only user-provided values
- merge the partial config with CLI overrides
- produce a resolved `Config` with defaults applied

### Global state

The resolved configuration should live in `src/globals.rs` alongside the existing global theme and buffer pool state.

Proposed API:

```rust
pub fn set_config(config: Config);
pub fn with_config<R>(f: impl FnOnce(Option<&Config>) -> R) -> R;
```

Responsibilities:

- store the resolved configuration once at startup
- provide read access to startup settings without threading CLI arguments through the application
- keep the active theme storage separate, because renderers still need the resolved `Theme`

### XDG config location

The config loader should search standard XDG config locations for the urvim TOML file. The expected path shape is:

- `$XDG_CONFIG_HOME/urvim/config.toml`
- then `$XDG_CONFIG_DIRS/urvim/config.toml` in order, using the first file that exists

If no file is found, the loader returns an empty partial config and startup continues with defaults.

## Data Models

### TOML config schema

The initial config file schema is intentionally small:

```toml
theme = "Friday Night"
```

Rules:

- `theme` is the first canonical field in the schema
- additional fields may be added later without changing the merge model
- unknown fields should be rejected so typos surface as startup errors

### Resolved config

`Config` represents the merged, startup-ready settings:

- `theme: String`

The resolved config is the value that gets stored globally and used by startup code after merge.

## Key Components

### Config loader

Responsibilities:

- find the config file in XDG search order
- read the file contents if present
- deserialize TOML into a partial config model
- surface file path, parse, and I/O errors clearly

Dependencies:

- XDG path resolution helper
- TOML deserialization
- filesystem I/O

### Config resolver

Responsibilities:

- merge file values with CLI overrides
- apply defaults for missing values
- produce the final `Config`

Dependencies:

- `PartialConfig`
- `Cli`

### Startup integration

Responsibilities:

- load config before the main editor loop starts
- store the resolved config globally
- use the resolved theme name to select the active runtime theme

Dependencies:

- `Config`
- `ThemeRegistry`
- `globals`

## User Interaction

There are no new interactive UI flows. The only user-facing behavior change is that startup can now read persistent settings from a TOML file and apply CLI overrides on top of them.

## External Dependencies

The implementation will need a crate or helper for XDG config path resolution, plus TOML parsing and deserialization, which the project already uses.

## Error Handling

The config loader should fail fast with specific errors:

- config file exists but cannot be opened
- config file exists but contains invalid TOML
- config file contains unknown fields or invalid schema data
- resolved theme name does not exist in the theme registry

The main startup path should print a clear error and exit before terminal mode if any of these fail.

## Security

The config file is local user preference data. The primary concern is safe filesystem access and clear error reporting, not secrets or authentication.

## Documentation

`docs/config.md` will serve as the user-facing summary of the canonical config schema, file location, and precedence rules. The implementation should treat the config module's public schema documentation as the source of truth and keep the docs page synchronized when config fields change.

## Component Interactions

1. CLI parsing collects startup overrides.
2. The config loader reads an optional TOML file from the XDG config directories.
3. The resolver merges file values with CLI values and applies defaults.
4. The resolved `Config` is stored in globals.
5. Theme startup code reads `Config.theme`, validates it against the registry, and stores the resolved `Theme` globally.
6. The editor loop uses the global config and theme for the rest of runtime behavior.

## Platform Considerations

The config lookup follows XDG conventions and should work consistently across supported desktop platforms. The search path logic should not depend on the current working directory.
