//! LSP configuration types shared between the editor and the LSP runtime.
//!
//! TOML parsing and validation remain in `urvim_core`.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Enabled inlay-hint kinds that can be configured through startup config.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InlayHintCapability {
    /// Enable type inlay hints.
    Type,
    /// Enable parameter inlay hints.
    Parameter,
}

/// The resolved LSP configuration used by the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct LspConfig {
    /// The resolved server configuration map keyed by server name.
    pub servers: BTreeMap<String, LspServerConfig>,
}

/// The resolved configuration for a single LSP server.
#[derive(Clone, Debug, PartialEq)]
pub struct LspServerConfig {
    /// Whether the server is enabled.
    pub enabled: bool,
    /// The executable command used to launch the server.
    pub command: String,
    /// Additional command-line arguments.
    pub args: Vec<String>,
    /// Environment variables passed to the server process.
    pub env: BTreeMap<String, String>,
    /// The filetypes that should attach to this server.
    pub filetypes: Vec<String>,
    /// The root markers used to discover workspace roots.
    pub root_markers: Vec<String>,
    /// Free-form server settings.
    pub settings: Value,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            filetypes: Vec::new(),
            root_markers: Vec::new(),
            settings: Value::Object(Default::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_server_config_default_is_disabled() {
        let config = LspServerConfig::default();
        assert!(!config.enabled);
        assert!(config.command.is_empty());
    }

    #[test]
    fn inlay_hint_capability_deserialize_type() {
        let value: InlayHintCapability = serde_json::from_str("\"type\"").unwrap();
        assert_eq!(value, InlayHintCapability::Type);
    }

    #[test]
    fn inlay_hint_capability_deserialize_parameter() {
        let value: InlayHintCapability = serde_json::from_str("\"parameter\"").unwrap();
        assert_eq!(value, InlayHintCapability::Parameter);
    }
}
