use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePath(PathBuf);

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
