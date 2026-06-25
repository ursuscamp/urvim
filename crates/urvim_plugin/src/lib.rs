//! Plugin system primitives.

mod contributions;
mod error;
pub mod manifest;
pub mod registry;
pub mod theme;

pub use contributions::{
    DynamicFiletype, DynamicPluginCommand, DynamicPluginTheme, DynamicPluginThemeSource,
    DynamicSyntaxProvider, PluginContributionRegistry, PluginEventKind, validate_contribution_name,
};
pub use error::PluginLoadError;
pub use manifest::{MANIFEST_FILE_NAME, PluginManifest, RawPluginManifest};
pub use registry::{LoadedPlugin, PluginConfigEntry, PluginRegistry};
pub use theme::{load_plugin_themes, load_theme_file};
