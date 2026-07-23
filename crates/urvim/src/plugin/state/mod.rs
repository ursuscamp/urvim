//! Persistent, plugin-owned BearScript state storage.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const STATE_RELATIVE_PATH: &str = "urvim/plugins";
const DEFAULT_STATE_HOME_SUFFIX: &str = ".local/state";

/// JSON-backed state isolated by plugin id.
#[derive(Default)]
pub struct PluginStateStore {
    root: Option<Result<PathBuf, String>>,
    loaded: RefCell<HashSet<String>>,
    values: RefCell<HashMap<String, HashMap<String, serde_json::Value>>>,
}

impl PluginStateStore {
    /// Creates a persistent store using the standard XDG state directory.
    pub fn from_environment() -> Self {
        Self {
            root: Some(state_root()),
            ..Self::default()
        }
    }

    /// Creates a persistent store rooted at an explicit directory.
    #[cfg(test)]
    pub fn at(root: PathBuf) -> Self {
        Self {
            root: Some(Ok(root)),
            ..Self::default()
        }
    }

    /// Returns a plugin-owned value.
    pub fn get(&self, plugin: &str, key: &str) -> Result<Option<serde_json::Value>, String> {
        self.ensure_loaded(plugin)?;
        Ok(self
            .values
            .borrow()
            .get(plugin)
            .and_then(|values| values.get(key))
            .cloned())
    }

    /// Sets and persists a plugin-owned value.
    pub fn set(&self, plugin: &str, key: String, value: serde_json::Value) -> Result<(), String> {
        self.ensure_loaded(plugin)?;
        let mut next = self
            .values
            .borrow()
            .get(plugin)
            .cloned()
            .unwrap_or_default();
        next.insert(key, value);
        self.persist(plugin, &next)?;
        self.values.borrow_mut().insert(plugin.to_string(), next);
        Ok(())
    }

    /// Deletes and persists a plugin-owned value.
    pub fn delete(&self, plugin: &str, key: &str) -> Result<bool, String> {
        self.ensure_loaded(plugin)?;
        let mut next = self
            .values
            .borrow()
            .get(plugin)
            .cloned()
            .unwrap_or_default();
        if next.remove(key).is_none() {
            return Ok(false);
        }
        self.persist(plugin, &next)?;
        self.values.borrow_mut().insert(plugin.to_string(), next);
        Ok(true)
    }

    /// Clears all values owned by a plugin and returns the number removed.
    pub fn clear(&self, plugin: &str) -> Result<usize, String> {
        self.ensure_loaded(plugin)?;
        let removed = self
            .values
            .borrow()
            .get(plugin)
            .map(HashMap::len)
            .unwrap_or(0);
        if removed == 0 {
            return Ok(0);
        }
        let next = HashMap::new();
        self.persist(plugin, &next)?;
        self.values.borrow_mut().insert(plugin.to_string(), next);
        Ok(removed)
    }

    fn ensure_loaded(&self, plugin: &str) -> Result<(), String> {
        if self.loaded.borrow().contains(plugin) {
            return Ok(());
        }

        let values = match self.path(plugin)? {
            None => HashMap::new(),
            Some(path) => match fs::read_to_string(&path) {
                Ok(text) => serde_json::from_str(&text).map_err(|error| {
                    format!("failed to parse plugin state {}: {error}", path.display())
                })?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
                Err(error) => {
                    return Err(format!(
                        "failed to read plugin state {}: {error}",
                        path.display()
                    ));
                }
            },
        };

        self.values.borrow_mut().insert(plugin.to_string(), values);
        self.loaded.borrow_mut().insert(plugin.to_string());
        Ok(())
    }

    fn persist(
        &self,
        plugin: &str,
        values: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let Some(path) = self.path(plugin)? else {
            return Ok(());
        };
        let parent = path
            .parent()
            .ok_or_else(|| format!("plugin state path {} has no parent", path.display()))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create plugin state directory {}: {error}",
                parent.display()
            )
        })?;
        let text = serde_json::to_string_pretty(values)
            .map_err(|error| format!("failed to serialize plugin state: {error}"))?;
        let temporary = temporary_path(&path);
        if let Err(error) = fs::write(&temporary, text) {
            return Err(format!(
                "failed to write plugin state {}: {error}",
                temporary.display()
            ));
        }
        if let Err(error) = fs::rename(&temporary, &path) {
            fs::remove_file(&temporary).ok();
            return Err(format!(
                "failed to replace plugin state {}: {error}",
                path.display()
            ));
        }
        Ok(())
    }

    fn path(&self, plugin: &str) -> Result<Option<PathBuf>, String> {
        self.root
            .as_ref()
            .map(|root| {
                root.as_ref()
                    .map(|root| root.join(format!("{plugin}.json")))
                    .map_err(Clone::clone)
            })
            .transpose()
    }
}

fn state_root() -> Result<PathBuf, String> {
    if let Some(value) = env::var_os("XDG_STATE_HOME")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value).join(STATE_RELATIVE_PATH));
    }
    let home = env::var_os("HOME").ok_or_else(|| {
        "HOME is not set and XDG_STATE_HOME is unavailable for plugin state".to_string()
    })?;
    Ok(PathBuf::from(home)
        .join(DEFAULT_STATE_HOME_SUFFIX)
        .join(STATE_RELATIVE_PATH))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        env::temp_dir().join(format!("urvim-plugin-state-{name}-{stamp}"))
    }

    #[test]
    fn state_persists_and_is_isolated_by_plugin() {
        let root = temp_root("persistence");
        let first = PluginStateStore::at(root.clone());
        first
            .set("one", "count".to_string(), serde_json::json!(3))
            .expect("state should persist");
        first
            .set("two", "count".to_string(), serde_json::json!(7))
            .expect("state should persist");

        let second = PluginStateStore::at(root.clone());
        assert_eq!(
            second.get("one", "count").unwrap(),
            Some(serde_json::json!(3))
        );
        assert_eq!(
            second.get("two", "count").unwrap(),
            Some(serde_json::json!(7))
        );
        assert!(second.delete("one", "count").unwrap());
        assert!(!second.delete("one", "count").unwrap());
        assert_eq!(second.clear("two").unwrap(), 1);
        assert_eq!(second.clear("two").unwrap(), 0);

        let third = PluginStateStore::at(root.clone());
        assert_eq!(third.get("one", "count").unwrap(), None);
        assert_eq!(third.get("two", "count").unwrap(), None);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn failed_persistence_does_not_change_cached_state() {
        let root = temp_root("rollback");
        let store = PluginStateStore::at(root.clone());
        store
            .set("demo", "value".to_string(), serde_json::json!("old"))
            .unwrap();
        fs::remove_file(root.join("demo.json")).unwrap();
        fs::remove_dir(&root).unwrap();
        fs::write(&root, "not a directory").unwrap();

        assert!(
            store
                .set("demo", "value".to_string(), serde_json::json!("new"))
                .is_err()
        );
        assert_eq!(
            store.get("demo", "value").unwrap(),
            Some(serde_json::json!("old"))
        );
        fs::remove_file(root).ok();
    }

    #[test]
    fn malformed_state_reports_a_clear_error() {
        let root = temp_root("malformed");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("demo.json"), "not json").unwrap();
        let store = PluginStateStore::at(root.clone());

        let error = store.get("demo", "key").unwrap_err();
        assert!(error.contains("failed to parse plugin state"));
        fs::remove_dir_all(root).ok();
    }
}
