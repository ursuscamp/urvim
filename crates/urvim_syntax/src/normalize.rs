use smol_str::SmolStr;

pub(super) fn normalize_label(label: &str) -> Option<SmolStr> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }

    Some(SmolStr::new(label.to_ascii_lowercase()))
}
