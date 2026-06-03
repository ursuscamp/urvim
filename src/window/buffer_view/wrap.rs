use super::BufferView;
use crate::buffer::display_grapheme_width;
use crate::config::WrapMode;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrappedLineSegment {
    pub start_byte: usize,
    pub end_byte: usize,
    pub is_continuation: bool,
}

#[derive(Debug, Clone, Copy)]
struct GraphemeSlice {
    start_byte: usize,
    end_byte: usize,
    width: usize,
    is_whitespace: bool,
}

impl BufferView {
    /// Splits one logical line into visual wrap segments.
    ///
    /// Algorithm outline:
    /// 1. Pre-tokenize the line into grapheme slices so we never split a grapheme cluster.
    /// 2. Grow one segment at a time until adding the next grapheme would overflow `max_width`.
    /// 3. In soft mode, remember the most recent whitespace/non-whitespace transition as a
    ///    preferred break point.
    /// 4. When overflow occurs:
    ///    - hard mode breaks at the overflow boundary,
    ///    - soft mode breaks at the latest remembered soft boundary when available,
    ///      otherwise it falls back to the same hard boundary.
    /// 5. Ensure forward progress by forcing at least one grapheme per segment, even when a
    ///    single grapheme is wider than `max_width`.
    pub(super) fn wrap_segments_for_line(
        line_text: &str,
        max_width: usize,
        mode: WrapMode,
    ) -> Vec<WrappedLineSegment> {
        if line_text.is_empty() {
            return vec![WrappedLineSegment {
                start_byte: 0,
                end_byte: 0,
                is_continuation: false,
            }];
        }

        let tab_width = crate::buffer::configured_tab_width();
        let graphemes = line_text
            .grapheme_indices(true)
            .map(|(start_byte, grapheme)| GraphemeSlice {
                start_byte,
                end_byte: start_byte + grapheme.len(),
                width: display_grapheme_width(grapheme, 0, tab_width),
                is_whitespace: grapheme.chars().all(char::is_whitespace),
            })
            .collect::<Vec<_>>();
        if graphemes.is_empty() {
            return vec![WrappedLineSegment {
                start_byte: 0,
                end_byte: 0,
                is_continuation: false,
            }];
        }

        let mut segments = Vec::new();
        let mut segment_start = 0usize;
        while segment_start < graphemes.len() {
            // Probe forward from `segment_start` to find where the current segment should end.
            let mut segment_width = 0usize;
            let mut overflow_index = graphemes.len();
            let mut idx = segment_start;
            let mut last_soft_break = None;
            while idx < graphemes.len() {
                let grapheme = graphemes[idx];
                let next_width = segment_width + grapheme.width;
                // Overflow after at least one grapheme: stop here and decide hard/soft break.
                if next_width > max_width && idx > segment_start {
                    overflow_index = idx;
                    break;
                }
                if next_width > max_width && idx == segment_start {
                    // Always progress by at least one grapheme even when one grapheme exceeds max_width.
                    idx += 1;
                    overflow_index = idx;
                    break;
                }

                segment_width = next_width;
                idx += 1;
                // Treat whitespace/non-whitespace transitions as soft-wrap candidates.
                if mode == WrapMode::Soft
                    && idx < graphemes.len()
                    && graphemes[idx - 1].is_whitespace != graphemes[idx].is_whitespace
                {
                    last_soft_break = Some(idx);
                }
            }

            let mut segment_end = overflow_index;
            // Soft mode prefers the latest boundary before overflow; if none exists we keep
            // `overflow_index` (hard fallback).
            if mode == WrapMode::Soft
                && overflow_index < graphemes.len()
                && let Some(soft_break) = last_soft_break
                && soft_break > segment_start
            {
                segment_end = soft_break;
            }

            // Safety fallback to avoid zero-length segments when bounds are degenerate.
            if segment_end <= segment_start {
                segment_end = (segment_start + 1).min(graphemes.len());
            }

            let start_byte = graphemes[segment_start].start_byte;
            let end_byte = if segment_end < graphemes.len() {
                graphemes[segment_end].start_byte
            } else {
                graphemes
                    .last()
                    .map(|grapheme| grapheme.end_byte)
                    .unwrap_or(line_text.len())
            };
            segments.push(WrappedLineSegment {
                start_byte,
                end_byte,
                is_continuation: !segments.is_empty(),
            });
            segment_start = segment_end;
        }

        segments
    }
}
