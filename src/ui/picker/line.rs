//! Small helpers for building formatted picker result rows.

use crate::syntax::FiletypeGlyph;
use crate::terminal::Style;
use crate::ui::line_format::{EllipsisPlacement, FormattedLineSection, LineSectionOverflow};
use std::path::Path;

/// Appends a filetype glyph and following spacer when a glyph is configured.
pub fn push_file_glyph(
    sections: &mut Vec<FormattedLineSection>,
    values: &mut Vec<String>,
    path: &Path,
    base_style: Style,
) {
    let Some(glyph) = FiletypeGlyph::from_path(path) else {
        return;
    };

    let glyph_width = unicode_width::UnicodeWidthStr::width(glyph.glyph.as_str()) as u16;
    sections.push(FormattedLineSection::fixed(
        glyph_width,
        base_style.accent(glyph.style),
    ));
    values.push(glyph.glyph.to_string());
    sections.push(FormattedLineSection::fixed(1, base_style));
    values.push(" ".to_string());
}

/// Appends a measured label that preserves the tail when truncated.
pub fn push_tail_label(
    sections: &mut Vec<FormattedLineSection>,
    values: &mut Vec<String>,
    label: String,
    style: Style,
) {
    sections.push(
        FormattedLineSection::measured(style)
            .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
    );
    values.push(label);
}

/// Appends a fixed-width text section.
pub fn push_fixed_text(
    sections: &mut Vec<FormattedLineSection>,
    values: &mut Vec<String>,
    text: String,
    style: Style,
) {
    let width = unicode_width::UnicodeWidthStr::width(text.as_str()) as u16;
    sections.push(FormattedLineSection::fixed(width, style));
    values.push(text);
}

/// Returns `path` relative to `root` when possible.
pub fn display_path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Returns `path` relative to the process current directory when possible.
pub fn display_path_relative_to_cwd(path: &Path) -> String {
    let Ok(cwd) = std::env::current_dir() else {
        return path.to_string_lossy().into_owned();
    };

    display_path_relative_to(cwd.as_path(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_path_relative_to_strips_root_prefix() {
        let root = Path::new("/tmp/project");
        let path = Path::new("/tmp/project/src/main.rs");

        assert_eq!(display_path_relative_to(root, path), "src/main.rs");
    }

    #[test]
    fn push_fixed_text_uses_display_width() {
        let mut sections = Vec::new();
        let mut values = Vec::new();

        push_fixed_text(
            &mut sections,
            &mut values,
            "ab".to_string(),
            Style::default(),
        );

        assert_eq!(sections.len(), 1);
        assert_eq!(values, vec!["ab"]);
    }
}
