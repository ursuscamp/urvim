use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;

/// Returns the user's home directory from the process environment.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Expands a leading `~` or `~/` using the user's home directory when available.
pub fn expand_home_path(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(path)
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePath(PathBuf);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_home_path_leaves_non_home_path_unchanged() {
        assert_eq!(
            expand_home_path("plugins/demo"),
            PathBuf::from("plugins/demo")
        );
    }

    #[test]
    fn expand_home_path_expands_home_prefix_when_home_is_available() {
        let Some(home) = home_dir() else {
            return;
        };

        assert_eq!(
            expand_home_path("~/plugins/demo"),
            home.join("plugins/demo")
        );
    }
}

impl AbsolutePath {
    pub fn new(path: PathBuf) -> Option<Self> {
        std::path::absolute(&path).ok().map(Self)
    }

    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        Self::new(path.to_path_buf())
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn display(&self) -> impl fmt::Display + fmt::Debug + '_ {
        self.0.display()
    }

    pub fn join(&self, path: impl AsRef<std::path::Path>) -> AbsolutePath {
        AbsolutePath(self.0.join(path))
    }
}

impl Deref for AbsolutePath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for AbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AbsolutePath").field(&self.0).finish()
    }
}

impl fmt::Display for AbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl From<AbsolutePath> for PathBuf {
    fn from(path: AbsolutePath) -> Self {
        path.0
    }
}

impl AsRef<std::path::Path> for AbsolutePath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
