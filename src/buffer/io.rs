use super::BufferCache;
use super::*;
use smol_str::SmolStr;

impl Buffer {
    /// Creates a new empty buffer.
    pub fn new() -> Self {
        let lines: Vector<Arc<str>> = Vector::unit(Arc::from(""));
        let syntax_name = SmolStr::new(crate::syntax::fallback_syntax_name());
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        Self {
            lines,
            saved_lines,
            path: None,
            syntax_generation: 0,
            syntax_background_generation: None,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
            ),
        }
    }

    /// Creates a buffer from a string slice.
    pub fn new_from_str(text: &str) -> Self {
        let lines: Vector<Arc<str>> = if text.is_empty() {
            Vector::unit(Arc::from(""))
        } else {
            text.lines().map(Arc::from).collect::<Vector<_>>()
        };
        let syntax_name =
            crate::syntax::resolve_builtin_syntax(None, lines.get(0).map(|line| line.as_ref()))
                .unwrap_or_else(|| SmolStr::new(crate::syntax::fallback_syntax_name()));
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        Self {
            lines,
            saved_lines,
            path: None,
            syntax_generation: 0,
            syntax_background_generation: None,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
            ),
        }
    }

    #[doc(hidden)]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self::new_from_str(text)
    }

    pub fn with_path(path: AbsolutePath) -> Self {
        let lines: Vector<Arc<str>> = Vector::unit(Arc::from(""));
        let syntax_name = crate::syntax::resolve_builtin_syntax(
            Some(path.as_path()),
            lines.get(0).map(|line| line.as_ref()),
        )
        .unwrap_or_else(|| SmolStr::new(crate::syntax::fallback_syntax_name()));
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        Self {
            lines,
            saved_lines,
            path: Some(path),
            syntax_generation: 0,
            syntax_background_generation: None,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
            ),
        }
    }

    pub fn from_str_with_path(text: &str, path: AbsolutePath) -> Self {
        let mut buf = Self::new_from_str(text);
        buf.set_path(path);
        buf
    }

    /// Loads a buffer from a file.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let abs_path = AbsolutePath::from_path(path).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "failed to resolve absolute path",
            )
        })?;
        Ok(Self::from_str_with_path(&contents, abs_path))
    }

    /// Saves the buffer contents to a file.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.as_str().as_bytes())?;
        Ok(())
    }
}
