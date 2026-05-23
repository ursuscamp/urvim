use super::BufferCache;
use super::*;
use smol_str::SmolStr;

impl Buffer {
    /// Creates a new empty buffer.
    pub fn new() -> Self {
        let lines = PieceTable::new_empty();
        let syntax_name = SmolStr::new(crate::syntax::fallback_syntax_name());
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        let markers = MarkersStore::with_line_count(lines.line_count());
        Self {
            lines,
            saved_lines,
            saved_disk_state: None,
            path: None,
            syntax_generation: 0,
            syntax_background_generation: None,
            indent_background_generation: None,
            visual_generation: 0,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            markers: markers.clone(),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
                markers,
            ),
        }
    }

    /// Creates a buffer from a string slice.
    pub fn new_from_str(text: &str) -> Self {
        let lines = PieceTable::from_text(text);
        let first_line = lines.line(0);
        let syntax_name = crate::syntax::resolve_builtin_syntax(
            None,
            first_line.as_ref().and_then(|line| line.contiguous_text()),
        )
        .unwrap_or_else(|| SmolStr::new(crate::syntax::fallback_syntax_name()));
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        let markers = MarkersStore::with_line_count(lines.line_count());
        Self {
            lines,
            saved_lines,
            saved_disk_state: None,
            path: None,
            syntax_generation: 0,
            syntax_background_generation: None,
            indent_background_generation: None,
            visual_generation: 0,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            markers: markers.clone(),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
                markers,
            ),
        }
    }

    #[doc(hidden)]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self::new_from_str(text)
    }

    pub fn with_path(path: AbsolutePath) -> Self {
        let lines = PieceTable::new_empty();
        let first_line = lines.line(0);
        let syntax_name = crate::syntax::resolve_builtin_syntax(
            Some(path.as_path()),
            first_line.as_ref().and_then(|line| line.contiguous_text()),
        )
        .unwrap_or_else(|| SmolStr::new(crate::syntax::fallback_syntax_name()));
        let saved_lines = lines.clone();
        let undo_lines = lines.clone();
        let markers = MarkersStore::with_line_count(lines.line_count());
        let buffer = Self {
            lines,
            saved_lines,
            saved_disk_state: None,
            path: Some(path),
            syntax_generation: 0,
            syntax_background_generation: None,
            indent_background_generation: None,
            visual_generation: 0,
            buffer_cache: BufferCache::new(syntax_name.clone()),
            markers: markers.clone(),
            undo_state: UndoState::new(
                undo_lines,
                Cursor::new(0, 0),
                BufferCache::new(syntax_name),
                markers,
            ),
        };
        buffer
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
        let mut buffer = Self::from_str_with_path(&contents, abs_path);
        buffer.mark_saved();
        Ok(buffer)
    }

    /// Saves the buffer contents to a file.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.as_str().as_bytes())?;
        Ok(())
    }
}
