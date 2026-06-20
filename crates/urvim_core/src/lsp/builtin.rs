use serde_json;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::config::{
    ConfigLoadError, LspConfig, LspServerConfig, PartialLspConfig, PartialLspServerConfig,
};

const BUILTIN_LSP_SOURCE: &str = include_str!("builtins.toml");

static BUILTIN_LSP_CONFIG: OnceLock<LspConfig> = OnceLock::new();

/// Returns the parsed builtin LSP configuration.
pub fn builtin_lsp_config() -> &'static LspConfig {
    BUILTIN_LSP_CONFIG.get_or_init(|| {
        let raw = toml::from_str::<PartialLspConfig>(BUILTIN_LSP_SOURCE)
            .unwrap_or_else(|error| panic!("invalid builtin LSP config: {error}"));
        validate_builtin_lsp_config(&raw)
            .unwrap_or_else(|error| panic!("invalid builtin LSP config: {error}"));
        resolve_builtin_lsp_config(&raw)
    })
}

fn resolve_builtin_lsp_config(raw: &PartialLspConfig) -> LspConfig {
    let mut servers = BTreeMap::new();

    for (name, server) in &raw.servers {
        servers.insert(name.clone(), resolve_builtin_server(server));
    }

    LspConfig { servers }
}

fn resolve_builtin_server(raw: &PartialLspServerConfig) -> LspServerConfig {
    let mut server = LspServerConfig::default();
    server.enabled = raw.enabled.unwrap_or(server.enabled);
    server.command = raw.command.clone().unwrap_or(server.command);
    server.args = raw.args.clone().unwrap_or(server.args);
    server.env = raw.env.clone().unwrap_or(server.env);
    server.filetypes = raw.filetypes.clone().unwrap_or(server.filetypes);
    server.root_markers = raw.root_markers.clone().unwrap_or(server.root_markers);
    server.settings = raw
        .settings
        .as_ref()
        .and_then(|v| serde_json::to_value(v).ok())
        .unwrap_or(server.settings);
    server
}

fn validate_builtin_lsp_config(raw: &PartialLspConfig) -> Result<(), ConfigLoadError> {
    if raw.servers.is_empty() {
        return Err(ConfigLoadError::Invalid {
            message: "builtin lsp config must define at least one server".to_string(),
        });
    }

    for (name, server) in &raw.servers {
        validate_builtin_lsp_server(name, server)?;
    }

    Ok(())
}

fn validate_builtin_lsp_server(
    name: &str,
    server: &PartialLspServerConfig,
) -> Result<(), ConfigLoadError> {
    if server
        .command
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(ConfigLoadError::Invalid {
            message: format!("builtin lsp {name}.command must not be empty"),
        });
    }

    Ok(())
}
