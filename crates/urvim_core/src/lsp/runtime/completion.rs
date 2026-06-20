//! Completion item conversion from raw LSP types to core UI types.
//!
//! Converts `CompletionResponse` → `Vec<CompletionCandidate>` and handles
//! ranking, deduplication, additional text edit resolution, and deprecated
//! flag detection. Pure conversion — no globals calls except
//! `position_to_cursor` for range mapping.

use lsp_types::{
    CompletionItem, CompletionItemTag, CompletionResponse, InsertTextFormat, PositionEncodingKind,
};

use crate::ui::completion::{CompletionCandidate, CompletionInsertFormat};
use urvim_text::{Cursor, PieceTable, TextObjectRange, TextRef, TextSnapshot};

use super::position_to_cursor;

pub(super) fn completion_response_to_candidates(
    response: CompletionResponse,
    lines: &PieceTable,
    cursor: Cursor,
    encoding: PositionEncodingKind,
) -> Vec<CompletionCandidate> {
    let items = match response {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };

    let query = current_word_prefix_text(lines, cursor);
    let mut items = items;
    rank_completion_items(&mut items, query.as_str());

    items
        .into_iter()
        .filter_map(|item| completion_item_to_candidate(item, lines, cursor, encoding.clone()))
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut deduped, item| {
            if let Some(existing) = deduped
                .iter_mut()
                .find(|existing| completion_candidate_same_identity(existing, &item))
            {
                if completion_candidate_score(&item) > completion_candidate_score(existing) {
                    *existing = item;
                }
            } else {
                deduped.push(item);
            }
            deduped
        })
}

fn rank_completion_items(items: &mut Vec<CompletionItem>, query: &str) {
    if query.trim().is_empty() {
        return;
    }

    let query = query.to_lowercase();
    items.retain(|item| {
        item.filter_text
            .as_deref()
            .unwrap_or(item.label.as_str())
            .to_lowercase()
            .starts_with(query.as_str())
    });
    items.sort_by(|left, right| {
        let left_sort = left
            .sort_text
            .as_deref()
            .unwrap_or(left.label.as_str())
            .to_lowercase();
        let right_sort = right
            .sort_text
            .as_deref()
            .unwrap_or(right.label.as_str())
            .to_lowercase();
        match left_sort.cmp(&right_sort) {
            std::cmp::Ordering::Equal => left.label.to_lowercase().cmp(&right.label.to_lowercase()),
            ordering => ordering,
        }
    });
}

fn completion_item_to_candidate(
    item: CompletionItem,
    lines: &PieceTable,
    cursor: Cursor,
    encoding: PositionEncodingKind,
) -> Option<CompletionCandidate> {
    let deprecated = completion_item_is_deprecated(&item);
    let additional_text_edits =
        completion_item_additional_text_edits(&item, lines, encoding.clone());
    let completion_item_json = serde_json::to_value(&item).ok();
    let label = item.label;
    let label_details = item.label_details;
    let (range, replacement) = match item.text_edit {
        Some(lsp_types::CompletionTextEdit::Edit(edit)) => (
            lsp_range_to_cursor_range(lines, &edit.range, encoding.clone())?,
            edit.new_text,
        ),
        Some(lsp_types::CompletionTextEdit::InsertAndReplace(edit)) => (
            lsp_range_to_cursor_range(lines, &edit.replace, encoding.clone())?,
            edit.new_text,
        ),
        None => {
            let replacement = item.insert_text.unwrap_or_else(|| label.clone());
            (current_word_range(lines, cursor), replacement)
        }
    };

    let mut candidate = CompletionCandidate::new(label, replacement, range, None);
    candidate.kind = item.kind;
    candidate.insert_format = item.insert_text_format.map(|format| match format {
        InsertTextFormat::PLAIN_TEXT => CompletionInsertFormat::PlainText,
        InsertTextFormat::SNIPPET => CompletionInsertFormat::Snippet,
        _ => CompletionInsertFormat::PlainText,
    });
    candidate.detail = item.detail;
    candidate.additional_text_edits = additional_text_edits;
    candidate.lsp_completion_item = completion_item_json;
    candidate.label_detail = label_details
        .as_ref()
        .and_then(|details| details.detail.clone());
    candidate.label_description = label_details
        .as_ref()
        .and_then(|details| details.description.clone());
    candidate.deprecated = deprecated;
    candidate.preselect = item.preselect.unwrap_or(false);

    Some(candidate)
}

fn lsp_range_to_cursor_range(
    lines: &PieceTable,
    range: &lsp_types::Range,
    encoding: PositionEncodingKind,
) -> Option<TextObjectRange> {
    Some(TextObjectRange {
        start: position_to_cursor(lines, range.start, encoding.clone())?,
        end: position_to_cursor(lines, range.end, encoding)?,
    })
}

fn completion_candidate_same_identity(
    left: &CompletionCandidate,
    right: &CompletionCandidate,
) -> bool {
    left.label == right.label
        && left.replacement == right.replacement
        && left.range == right.range
        && left.kind == right.kind
        && left.symbol == right.symbol
        && left.insert_format == right.insert_format
}

fn completion_candidate_score(candidate: &CompletionCandidate) -> usize {
    candidate.additional_text_edits.len() + usize::from(candidate.lsp_completion_item.is_some())
}

pub(super) fn completion_item_additional_text_edits(
    item: &CompletionItem,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<crate::ui::completion::CompletionTextEdit> {
    item.additional_text_edits
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|edit| {
            lsp_range_to_cursor_range(lines, &edit.range, encoding.clone()).map(|range| {
                crate::ui::completion::CompletionTextEdit {
                    range,
                    text: edit.new_text.clone(),
                }
            })
        })
        .collect()
}

fn current_word_prefix_text(lines: &PieceTable, cursor: Cursor) -> String {
    let Some(line) = lines.line(cursor.line) else {
        return String::new();
    };
    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        start = prev_start;
    }

    line.range_text(start, cursor_col).unwrap_or_default()
}

fn completion_item_is_deprecated(item: &CompletionItem) -> bool {
    if item.deprecated.unwrap_or(false) {
        return true;
    }

    item.tags
        .as_ref()
        .is_some_and(|tags| tags.contains(&CompletionItemTag::DEPRECATED))
}

fn current_word_range(lines: &PieceTable, cursor: Cursor) -> TextObjectRange {
    let Some(line) = lines.line(cursor.line) else {
        return TextObjectRange {
            start: cursor,
            end: cursor,
        };
    };
    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        start = prev_start;
    }

    TextObjectRange {
        start: Cursor::new(cursor.line, start),
        end: Cursor::new(cursor.line, cursor_col),
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_snapshot(text: &str) -> PieceTable {
        PieceTable::from_text(text)
    }

    #[test]
    fn completion_item_deprecated_uses_flag_or_tags() {
        let flagged = CompletionItem {
            label: "flagged".to_string(),
            deprecated: Some(true),
            ..CompletionItem::default()
        };
        assert!(completion_item_is_deprecated(&flagged));

        let tagged = CompletionItem {
            label: "tagged".to_string(),
            tags: Some(vec![CompletionItemTag::DEPRECATED]),
            ..CompletionItem::default()
        };
        assert!(completion_item_is_deprecated(&tagged));

        let plain = CompletionItem {
            label: "plain".to_string(),
            ..CompletionItem::default()
        };
        assert!(!completion_item_is_deprecated(&plain));
    }

    #[test]
    fn completion_response_uses_text_edits_and_insert_text() {
        let mut edit_item =
            lsp_types::CompletionItem::new_simple("edit".to_string(), "".to_string());
        edit_item.text_edit = Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(0, 5),
            },
            new_text: "hi".to_string(),
        }));

        let mut insert_item =
            lsp_types::CompletionItem::new_simple("insert".to_string(), "".to_string());
        insert_item.insert_text = Some("earth".to_string());

        let response = lsp_types::CompletionResponse::Array(vec![edit_item, insert_item]);
        let items = completion_response_to_candidates(
            response,
            &line_snapshot("hello world"),
            Cursor::new(0, 0),
            PositionEncodingKind::UTF8,
        );

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].replacement, "hi");
        assert_eq!(items[1].replacement, "earth");
        assert!(items[0].lsp_completion_item.is_some());
        assert!(items[1].lsp_completion_item.is_some());
    }

    #[test]
    fn completion_response_prefers_items_with_additional_edits_when_labels_match() {
        let plain = lsp_types::CompletionItem::new_simple("width".to_string(), "width".to_string());
        let mut imported =
            lsp_types::CompletionItem::new_simple("width".to_string(), "width".to_string());
        imported.additional_text_edits = Some(vec![lsp_types::TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(0, 0),
            },
            new_text: "use foo::Width;\n".to_string(),
        }]);

        let items = completion_response_to_candidates(
            CompletionResponse::Array(vec![plain, imported]),
            &line_snapshot(""),
            Cursor::new(0, 0),
            PositionEncodingKind::UTF8,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "width");
        assert_eq!(items[0].additional_text_edits.len(), 1);
    }
}
