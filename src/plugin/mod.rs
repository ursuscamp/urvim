//! Plugin system primitives.
//!
//! This module currently supports manifest-only plugins. Process-backed plugins
//! will build on the same manifest and registry layers later.

pub mod codec;
mod error;
pub mod manifest;
pub mod protocol;
pub mod registry;
pub mod runtime;
pub mod theme;

pub use codec::{decode_frame, encode_frame, read_frame, write_frame};
pub use error::PluginLoadError;
pub use manifest::{
    MANIFEST_FILE_NAME, PluginCommand, PluginManifest, PluginProcess, RawPluginManifest,
};
pub use protocol::{PluginMessage, PluginNotification, PluginRequest, PluginResponse};
pub use registry::{LoadedPlugin, PluginRegistry};
pub use runtime::PluginRuntimeEvent;
pub use runtime::{
    PluginProcessRuntime, PluginProcessState, PluginProcessStatus, PluginRuntime, PluginStatusEntry,
};
pub use theme::load_plugin_themes;
