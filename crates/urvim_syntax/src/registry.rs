use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use super::builtin::builtin_syntax_definitions;
use super::definition::*;
use super::error::SyntaxLoadError;
use super::normalize::normalize_label;

const FALLBACK_SYNTAX_NAME: &str = "plaintext";

/// In-memory registry of built-in syntax definitions.
#[derive(Debug)]
pub struct SyntaxRegistry {
    entries: BTreeMap<String, Arc<SyntaxDefinition>>,
    aliases: BTreeMap<String, SmolStr>,
    filename_patterns: BTreeMap<String, SmolStr>,
    shebang_patterns: BTreeMap<String, SmolStr>,
}

impl SyntaxRegistry {
    /// Loads all built-in syntax definitions from code-defined metadata.
    pub fn load_builtin() -> Result<Self, SyntaxLoadError> {
        let mut registry = Self {
            entries: BTreeMap::new(),
            aliases: BTreeMap::new(),
            filename_patterns: BTreeMap::new(),
            shebang_patterns: BTreeMap::new(),
        };

        for definition in builtin_syntax_definitions()? {
            registry.insert(definition)?;
        }

        Ok(registry)
    }

    /// Returns the registered syntax names in sorted order.
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Looks up a syntax definition by name or alias.
    pub fn get_by_name(&self, name: &str) -> Option<Arc<SyntaxDefinition>> {
        let canonical = self.resolve_label(name)?;
        self.entries.get(canonical.as_str()).cloned()
    }

    /// Looks up a syntax name by shebang or filename.
    pub fn resolve_for_input(&self, path: Option<&Path>, shebang: Option<&str>) -> Option<SmolStr> {
        if let Some(shebang) = shebang
            && let Some(definition) = self.resolve_for_shebang(shebang)
        {
            return Some(definition);
        }

        if let Some(path) = path
            && let Some(definition) = self.resolve_for_filename(path)
        {
            return Some(definition);
        }

        Some(SmolStr::new(FALLBACK_SYNTAX_NAME))
    }

    /// Resolves a canonical syntax name from its canonical name or alias label.
    pub fn resolve_label(&self, label: &str) -> Option<SmolStr> {
        let label = normalize_label(label)?;
        if self.entries.contains_key(label.as_str()) {
            return Some(label);
        }

        self.aliases.get(label.as_str()).cloned()
    }

    /// Returns the display label for a resolved syntax name or alias.
    pub fn display_name(&self, name: &str) -> Option<SmolStr> {
        let canonical = self.resolve_label(name)?;
        self.entries
            .get(canonical.as_str())
            .map(|definition| definition.metadata.display_name.clone())
    }

    /// Returns the compiled metadata for a resolved syntax name or alias.
    pub fn metadata(&self, name: &str) -> Option<SyntaxMetadata> {
        let canonical = self.resolve_label(name)?;
        self.entries
            .get(canonical.as_str())
            .map(|definition| definition.metadata.clone())
    }

    fn insert(&mut self, definition: SyntaxDefinition) -> Result<(), SyntaxLoadError> {
        if definition.metadata.name.trim().is_empty() {
            return Err(SyntaxLoadError::InvalidSyntaxName(
                definition.metadata.name.to_string(),
            ));
        }

        let name = definition.metadata.name.clone();
        if self.entries.contains_key(name.as_str()) {
            return Err(SyntaxLoadError::DuplicateSyntaxName(name.to_string()));
        }

        for alias in &definition.metadata.alias {
            if self.entries.contains_key(alias.as_str()) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "alias".to_string(),
                    pattern: alias.to_string(),
                    first: alias.to_string(),
                    second: definition.metadata.name.to_string(),
                });
            }
            if let Some(existing) = self.aliases.get(alias.as_str()) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "alias".to_string(),
                    pattern: alias.to_string(),
                    first: existing.to_string(),
                    second: definition.metadata.name.to_string(),
                });
            }
        }

        for pattern in &definition.metadata.filename {
            let pattern = pattern.as_str().to_string();
            if let Some(existing) = self.filename_patterns.get(&pattern) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "filename".to_string(),
                    pattern,
                    first: existing.to_string(),
                    second: definition.metadata.name.to_string(),
                });
            }
        }

        for pattern in &definition.metadata.shebang {
            let pattern = pattern.as_str().to_string();
            if let Some(existing) = self.shebang_patterns.get(&pattern) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "shebang".to_string(),
                    pattern,
                    first: existing.to_string(),
                    second: definition.metadata.name.to_string(),
                });
            }
        }

        for alias in &definition.metadata.alias {
            self.aliases.insert(alias.to_string(), name.clone());
        }
        for pattern in &definition.metadata.filename {
            self.filename_patterns
                .insert(pattern.as_str().to_string(), name.clone());
        }
        for pattern in &definition.metadata.shebang {
            self.shebang_patterns
                .insert(pattern.as_str().to_string(), name.clone());
        }
        self.entries.insert(name.to_string(), Arc::new(definition));
        Ok(())
    }

    fn resolve_for_filename(&self, path: &Path) -> Option<SmolStr> {
        let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        let file_name = file_name.as_str();

        self.entries
            .iter()
            .find(|(_, definition)| {
                definition
                    .metadata
                    .filename
                    .iter()
                    .any(|pattern| pattern.is_match(file_name))
            })
            .map(|(_, definition)| definition.metadata.name.clone())
    }

    fn resolve_for_shebang(&self, shebang: &str) -> Option<SmolStr> {
        self.entries
            .iter()
            .find(|(_, definition)| {
                definition
                    .metadata
                    .shebang
                    .iter()
                    .any(|pattern| pattern.is_match(shebang))
            })
            .map(|(_, definition)| definition.metadata.name.clone())
    }
}

/// Returns the lazily loaded built-in syntax registry.
pub fn builtin_syntax_registry() -> Result<&'static SyntaxRegistry, SyntaxLoadError> {
    static REGISTRY: OnceLock<Result<SyntaxRegistry, SyntaxLoadError>> = OnceLock::new();
    match REGISTRY.get_or_init(SyntaxRegistry::load_builtin) {
        Ok(registry) => Ok(registry),
        Err(error) => Err(error.clone()),
    }
}

/// Returns the canonical fallback syntax name.
pub fn fallback_syntax_name() -> &'static str {
    FALLBACK_SYNTAX_NAME
}

/// Resolves the best matching built-in syntax definition for the provided input.
pub fn resolve_builtin_syntax(path: Option<&Path>, shebang: Option<&str>) -> Option<SmolStr> {
    builtin_syntax_registry()
        .ok()
        .and_then(|registry| registry.resolve_for_input(path, shebang))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn builtin_registry_loads_code_defined_syntaxes() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        assert!(!registry.names().is_empty());
        assert_eq!(
            registry
                .resolve_for_input(Some(Path::new("src/main.rs")), None)
                .as_deref(),
            Some("rust")
        );
        assert_eq!(
            registry
                .resolve_for_input(Some(Path::new("README.md")), None)
                .as_deref(),
            Some("markdown")
        );
        assert_eq!(
            registry.resolve_label("javascript").as_deref(),
            Some("javascript")
        );
        assert_eq!(registry.resolve_label("js").as_deref(), Some("javascript"));
        assert_eq!(registry.display_name("rust").as_deref(), Some("Rust"));
        assert_eq!(
            registry
                .metadata("rust")
                .expect("rust metadata should exist")
                .comment_prefix
                .as_deref(),
            Some("//")
        );
    }

    #[test]
    fn resolve_label_matches_aliases_case_insensitively() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        assert_eq!(registry.resolve_label("JS").as_deref(), Some("javascript"));
        assert_eq!(registry.resolve_label("Md").as_deref(), Some("markdown"));
    }

    #[test]
    fn builtin_registry_returns_shared_definitions() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        let rust = registry
            .get_by_name("rust")
            .expect("rust syntax should exist");
        assert_eq!(rust.name(), "rust");

        let rust_again = registry
            .get_by_name("rs")
            .expect("rust alias should resolve");
        assert!(Arc::ptr_eq(&rust, &rust_again));
    }
}
