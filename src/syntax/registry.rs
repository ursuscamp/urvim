use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use super::builtin::builtin_syntax_sources;
use super::definition::*;
use super::error::SyntaxLoadError;
use super::loader::{parse_syntax, resolve_metadata, resolve_syntax};
use super::normalize::normalize_label;

const FALLBACK_SYNTAX_NAME: &str = "plaintext";

#[derive(Debug, Clone)]
struct SyntaxEntry {
    source_name: String,
    raw: RawSyntaxDefinition,
    metadata: SyntaxMetadata,
    compiled: Option<Arc<SyntaxDefinition>>,
}

impl SyntaxEntry {
    fn from_raw(
        source_name: impl Into<String>,
        raw: RawSyntaxDefinition,
    ) -> Result<Self, SyntaxLoadError> {
        let metadata = resolve_metadata(&raw.metadata)?;
        Ok(Self {
            source_name: source_name.into(),
            raw,
            metadata,
            compiled: None,
        })
    }
}

/// In-memory registry of resolved syntax definitions.
#[derive(Debug)]
pub struct SyntaxRegistry {
    entries: RwLock<BTreeMap<String, SyntaxEntry>>,
    aliases: BTreeMap<String, SmolStr>,
    filename_patterns: BTreeMap<String, SmolStr>,
    shebang_patterns: BTreeMap<String, SmolStr>,
}

impl SyntaxRegistry {
    /// Loads all built-in syntax definitions from embedded TOML sources.
    pub fn load_builtin() -> Result<Self, SyntaxLoadError> {
        let builtin_sources = builtin_syntax_sources();

        let mut registry = Self {
            entries: RwLock::new(BTreeMap::new()),
            aliases: BTreeMap::new(),
            filename_patterns: BTreeMap::new(),
            shebang_patterns: BTreeMap::new(),
        };

        for (source_name, source) in builtin_sources {
            let raw = parse_syntax(source_name, source)?;
            let entry = SyntaxEntry::from_raw(source_name, raw)?;
            registry.insert(entry)?;
        }

        registry.validate_references()?;
        Ok(registry)
    }

    /// Returns the registered syntax names in sorted order.
    pub fn names(&self) -> Vec<String> {
        self.entries
            .read()
            .expect("syntax registry lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    /// Looks up a compiled syntax definition by name, promoting it on demand.
    pub fn get_by_name(&self, name: &str) -> Option<Arc<SyntaxDefinition>> {
        self.promote(name).ok()
    }

    /// Looks up a syntax name by shebang or filename without promoting it.
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
        let entries = self.entries.read().expect("syntax registry lock poisoned");
        if entries.contains_key(label.as_str()) {
            return Some(label);
        }

        self.aliases.get(label.as_str()).cloned()
    }

    /// Returns the display label for a resolved syntax name or alias.
    pub fn display_name(&self, name: &str) -> Option<SmolStr> {
        let canonical = self.resolve_label(name)?;
        let entries = self.entries.read().expect("syntax registry lock poisoned");
        entries
            .get(canonical.as_str())
            .map(|entry| entry.metadata.display_name.clone())
    }

    /// Promotes a raw syntax definition to its compiled form.
    pub fn promote(&self, name: &str) -> Result<Arc<SyntaxDefinition>, SyntaxLoadError> {
        let canonical = self
            .resolve_label(name)
            .ok_or_else(|| SyntaxLoadError::InvalidSyntaxName(name.to_string()))?;

        {
            let entries = self.entries.read().expect("syntax registry lock poisoned");
            if let Some(compiled) = entries
                .get(canonical.as_str())
                .and_then(|entry| entry.compiled.clone())
            {
                return Ok(compiled);
            }
        }

        let mut entries = self.entries.write().expect("syntax registry lock poisoned");
        let entry = entries
            .get_mut(canonical.as_str())
            .ok_or_else(|| SyntaxLoadError::InvalidSyntaxName(canonical.to_string()))?;

        if let Some(compiled) = entry.compiled.clone() {
            return Ok(compiled);
        }

        let definition = Arc::new(resolve_syntax(entry.raw.clone(), &entry.source_name)?);
        entry.compiled = Some(definition.clone());
        Ok(definition)
    }

    /// Inserts a parsed builtin syntax definition into the registry.
    fn insert(&mut self, entry: SyntaxEntry) -> Result<(), SyntaxLoadError> {
        if entry.metadata.name.trim().is_empty() {
            return Err(SyntaxLoadError::InvalidSyntaxName(
                entry.metadata.name.to_string(),
            ));
        }

        let name = entry.metadata.name.clone();
        let entries = self.entries.read().expect("syntax registry lock poisoned");
        if entries.contains_key(name.as_str()) {
            return Err(SyntaxLoadError::DuplicateSyntaxName(name.to_string()));
        }

        for alias in &entry.metadata.alias {
            if entries.contains_key(alias.as_str()) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "alias".to_string(),
                    pattern: alias.to_string(),
                    first: alias.to_string(),
                    second: entry.metadata.name.to_string(),
                });
            }
            if let Some(existing) = self.aliases.get(alias.as_str()) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "alias".to_string(),
                    pattern: alias.to_string(),
                    first: existing.to_string(),
                    second: entry.metadata.name.to_string(),
                });
            }
        }

        for pattern in &entry.metadata.filename {
            let pattern = pattern.as_str().to_string();
            if let Some(existing) = self.filename_patterns.get(&pattern) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "filename".to_string(),
                    pattern,
                    first: existing.to_string(),
                    second: entry.metadata.name.to_string(),
                });
            }
        }

        for pattern in &entry.metadata.shebang {
            let pattern = pattern.as_str().to_string();
            if let Some(existing) = self.shebang_patterns.get(&pattern) {
                return Err(SyntaxLoadError::DuplicateMetadataMapping {
                    field: "shebang".to_string(),
                    pattern,
                    first: existing.to_string(),
                    second: entry.metadata.name.to_string(),
                });
            }
        }

        drop(entries);

        for alias in &entry.metadata.alias {
            self.aliases.insert(alias.to_string(), name.clone());
        }
        for pattern in &entry.metadata.filename {
            self.filename_patterns
                .insert(pattern.as_str().to_string(), name.clone());
        }
        for pattern in &entry.metadata.shebang {
            self.shebang_patterns
                .insert(pattern.as_str().to_string(), name.clone());
        }
        self.entries
            .write()
            .expect("syntax registry lock poisoned")
            .insert(name.to_string(), entry);
        Ok(())
    }

    fn resolve_for_filename(&self, path: &Path) -> Option<SmolStr> {
        let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        let file_name = file_name.as_str();

        self.entries
            .read()
            .expect("syntax registry lock poisoned")
            .iter()
            .find(|(_, entry)| {
                entry
                    .metadata
                    .filename
                    .iter()
                    .any(|pattern| pattern.is_match(file_name))
            })
            .map(|(_, entry)| entry.metadata.name.clone())
    }

    fn resolve_for_shebang(&self, shebang: &str) -> Option<SmolStr> {
        self.entries
            .read()
            .expect("syntax registry lock poisoned")
            .iter()
            .find(|(_, entry)| {
                entry
                    .metadata
                    .shebang
                    .iter()
                    .any(|pattern| pattern.is_match(shebang))
            })
            .map(|(_, entry)| entry.metadata.name.clone())
    }

    fn validate_references(&self) -> Result<(), SyntaxLoadError> {
        let entries = self.entries.read().expect("syntax registry lock poisoned");
        for (syntax_name, entry) in entries.iter() {
            for rule in &entry.raw.rules {
                self.validate_rule_references(syntax_name, rule)?;
            }
        }

        Ok(())
    }

    fn validate_rule_references(
        &self,
        syntax_name: &str,
        rule: &RawRule,
    ) -> Result<(), SyntaxLoadError> {
        match rule {
            RawRule::Regex { .. } => Ok(()),
            RawRule::Injection {
                selector: RawInjectionSelector::Static { name },
                ..
            } => {
                if self.resolve_label(name).is_none() {
                    return Err(SyntaxLoadError::UnknownNestedSyntax {
                        syntax: syntax_name.to_string(),
                        nested: name.to_string(),
                    });
                }
                Ok(())
            }
            RawRule::Injection {
                selector: RawInjectionSelector::Capture { .. },
                ..
            } => Ok(()),
        }
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
    fn builtin_registry_loads_raw_syntaxes_without_promoting_them() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        let entries = registry
            .entries
            .read()
            .expect("syntax registry lock poisoned");
        assert!(!entries.is_empty());
        assert!(entries.values().all(|entry| entry.compiled.is_none()));
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
            entries
                .get("rust")
                .expect("rust syntax should exist")
                .raw
                .metadata
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
    fn builtin_syntaxes_use_ordered_rules_or_empty_metadata() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        let entries = registry
            .entries
            .read()
            .expect("syntax registry lock poisoned");

        for (name, entry) in entries.iter() {
            if name.as_str() != "plaintext" {
                assert!(
                    !entry.raw.rules.is_empty(),
                    "syntax {name} should define at least one ordered rule"
                );
            }
        }
    }

    #[test]
    fn builtin_registry_promotes_syntaxes_on_demand() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        let rust = registry
            .promote("rust")
            .expect("rust syntax should promote");
        assert_eq!(rust.name(), "rust");

        let entries = registry
            .entries
            .read()
            .expect("syntax registry lock poisoned");
        let rust_entry = entries.get("rust").expect("rust syntax should exist");
        assert!(rust_entry.compiled.is_some());
        assert!(
            entries
                .iter()
                .filter(|(name, _)| name.as_str() != "rust")
                .all(|(_, entry)| entry.compiled.is_none())
        );

        let rust_again = registry
            .promote("rust")
            .expect("rust syntax should promote");
        assert!(Arc::ptr_eq(&rust, &rust_again));
    }

    #[test]
    fn builtin_registry_promotes_nested_syntaxes_only_when_requested() {
        let registry = SyntaxRegistry::load_builtin().expect("syntax registry should load");
        assert_eq!(registry.resolve_label("js").as_deref(), Some("javascript"));

        let entries = registry
            .entries
            .read()
            .expect("syntax registry lock poisoned");
        assert!(
            entries
                .get("javascript")
                .expect("javascript syntax should exist")
                .compiled
                .is_none()
        );
    }
}
