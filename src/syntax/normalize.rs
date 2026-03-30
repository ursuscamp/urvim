use smol_str::SmolStr;

use super::error::SyntaxLoadError;

pub(super) fn normalize_label(label: &str) -> Option<SmolStr> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }

    Some(SmolStr::new(label.to_ascii_lowercase()))
}

pub(super) fn normalize_context_marker(
    syntax: &str,
    marker: &str,
) -> Result<SmolStr, SyntaxLoadError> {
    normalize_label(marker).ok_or_else(|| SyntaxLoadError::InvalidContextMarker {
        syntax: syntax.to_string(),
        marker: marker.to_string(),
    })
}
