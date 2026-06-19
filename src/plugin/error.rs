use std::fmt;
use std::path::PathBuf;

use crate::theme::ThemeLoadError;

/// Errors raised while loading or validating plugin manifests.
#[derive(Debug)]
pub enum PluginLoadError {
    /// The plugin manifest could not be read from disk.
    Io {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The plugin manifest could not be parsed as TOML.
    Parse {
        /// The manifest source name or path.
        source: String,
        /// The parser error message.
        message: String,
    },
    /// A manifest field failed validation.
    Invalid {
        /// Human-readable validation failure.
        message: String,
    },
    /// A configured plugin failed to load.
    Plugin {
        /// The configured plugin id.
        id: String,
        /// The resolved plugin directory.
        path: PathBuf,
        /// The underlying plugin load error.
        source: Box<PluginLoadError>,
    },
    /// A plugin-provided theme failed to load.
    Theme {
        /// The plugin that contributed the theme path.
        plugin: String,
        /// The theme file path.
        path: PathBuf,
        /// The underlying theme load error.
        source: ThemeLoadError,
    },
    /// A MessagePack protocol frame failed to encode or decode.
    Protocol {
        /// Human-readable protocol error.
        message: String,
    },
    /// A process-backed plugin runtime failed.
    Runtime {
        /// Human-readable runtime error.
        message: String,
    },
}

impl PluginLoadError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }
}

impl fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "failed to read plugin manifest {}: {source}",
                    path.display()
                )
            }
            Self::Parse { source, message } => {
                write!(f, "failed to parse plugin manifest {source}: {message}")
            }
            Self::Invalid { message } => write!(f, "invalid plugin manifest: {message}"),
            Self::Plugin { id, path, source } => write!(
                f,
                "failed to load plugin {id:?} from {}: {source}",
                path.display()
            ),
            Self::Theme {
                plugin,
                path,
                source,
            } => write!(
                f,
                "failed to load theme for plugin {plugin:?} from {}: {source}",
                path.display()
            ),
            Self::Protocol { message } => write!(f, "plugin protocol error: {message}"),
            Self::Runtime { message } => write!(f, "plugin runtime error: {message}"),
        }
    }
}

impl std::error::Error for PluginLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Plugin { source, .. } => Some(source.as_ref()),
            Self::Theme { source, .. } => Some(source),
            Self::Parse { .. }
            | Self::Invalid { .. }
            | Self::Protocol { .. }
            | Self::Runtime { .. } => None,
        }
    }
}
