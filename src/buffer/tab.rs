use super::*;
use crate::config::TabInsertion;

impl Buffer {
    /// Infers the buffer's indentation style from the first clear leading-whitespace prefix.
    pub fn inferred_tab_insertion(&self) -> Option<TabInsertion> {
        for line in self.lines.iter() {
            if let Some(style) = inferred_line_tab_insertion(line.as_ref()) {
                return Some(style);
            }
        }

        None
    }
}

fn inferred_line_tab_insertion(line: &str) -> Option<TabInsertion> {
    let mut saw_space = false;
    let mut saw_tab = false;

    for ch in line.chars() {
        match ch {
            ' ' => {
                saw_space = true;
                if saw_tab {
                    return None;
                }
            }
            '\t' => {
                saw_tab = true;
                if saw_space {
                    return None;
                }
            }
            _ => break,
        }
    }

    if saw_tab {
        Some(TabInsertion::Tabs)
    } else if saw_space {
        Some(TabInsertion::Spaces)
    } else {
        None
    }
}
