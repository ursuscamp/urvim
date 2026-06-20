//! Reusable styled line formatting.

use crate::ui::text_width::{ClipSide, EllipsisSide, clip_text, display_width, ellipsize_text};
use urvim_terminal::Style;

/// Error returned when a line template is rendered with the wrong number of values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineFormatError {
    /// The template and value counts do not match.
    MismatchedSectionCount { expected: usize, actual: usize },
    /// A flex section used an invalid weight.
    InvalidFlexWeight { section: usize },
}

/// Rendered text segment with a resolved style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedLineSegment {
    /// Segment text.
    pub text: String,
    /// Segment style.
    pub style: Style,
}

impl FormattedLineSegment {
    /// Creates a rendered line segment.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Width policy for a formatted line section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineSectionWidth {
    /// Use an exact width.
    Fixed(u16),
    /// Use the rendered width of the provided value.
    Measured,
    /// Share remaining width proportionally by weight.
    Flex(u16),
}

impl LineSectionWidth {
    /// Creates a fixed-width section.
    pub fn fixed(width: u16) -> Self {
        Self::Fixed(width)
    }

    /// Creates a measured-width section.
    pub fn measured() -> Self {
        Self::Measured
    }

    /// Creates a flex-width section.
    pub fn flex(weight: u16) -> Self {
        Self::Flex(weight)
    }
}

/// Alignment used within a section's allocated width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineSectionAlignment {
    /// Align text to the left.
    #[default]
    Left,
    /// Center text within the width.
    Center,
    /// Align text to the right.
    Right,
}

/// Ellipsis placement used when content must be truncated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EllipsisPlacement {
    /// Place the ellipsis at the start and keep the suffix.
    Start,
    /// Place the ellipsis in the middle and keep both ends.
    Middle,
    /// Place the ellipsis at the end and keep the prefix.
    #[default]
    End,
}

/// Overflow behavior used when content exceeds an allocated width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineSectionOverflow {
    /// Remove the overflowing text.
    Clip,
    /// Replace trimmed text with an ellipsis.
    Ellipsis(EllipsisPlacement),
}

impl Default for LineSectionOverflow {
    fn default() -> Self {
        Self::Clip
    }
}

/// Template section describing one value in a formatted line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedLineSection {
    /// Style applied to the section.
    pub style: Style,
    /// Width policy for the section.
    pub width: LineSectionWidth,
    /// Alignment applied inside the allocated width.
    pub alignment: LineSectionAlignment,
    /// Overflow behavior for content wider than the allocated width.
    pub overflow: LineSectionOverflow,
}

impl FormattedLineSection {
    /// Creates a fixed-width section.
    pub fn fixed(width: u16, style: Style) -> Self {
        Self {
            style,
            width: LineSectionWidth::Fixed(width),
            alignment: LineSectionAlignment::Left,
            overflow: LineSectionOverflow::Clip,
        }
    }

    /// Creates a measured-width section.
    pub fn measured(style: Style) -> Self {
        Self {
            style,
            width: LineSectionWidth::Measured,
            alignment: LineSectionAlignment::Left,
            overflow: LineSectionOverflow::Clip,
        }
    }

    /// Creates a flex-width section.
    pub fn flex(weight: u16, style: Style) -> Self {
        Self {
            style,
            width: LineSectionWidth::Flex(weight),
            alignment: LineSectionAlignment::Left,
            overflow: LineSectionOverflow::Clip,
        }
    }

    /// Sets the section alignment.
    pub fn with_alignment(mut self, alignment: LineSectionAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets the section overflow behavior.
    pub fn with_overflow(mut self, overflow: LineSectionOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

/// Stored line template that formats a matching group of values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedLineTemplate {
    sections: Vec<FormattedLineSection>,
}

impl FormattedLineTemplate {
    /// Creates a new line template.
    pub fn new(sections: Vec<FormattedLineSection>) -> Self {
        Self { sections }
    }

    /// Returns the template sections.
    pub fn sections(&self) -> &[FormattedLineSection] {
        self.sections.as_slice()
    }

    /// Returns the number of sections in the template.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Renders the template into styled segments.
    pub fn render_segments<I, S>(
        &self,
        values: I,
        available_width: u16,
    ) -> Result<Vec<FormattedLineSegment>, LineFormatError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let values = values
            .into_iter()
            .map(|value| value.as_ref().to_string())
            .collect::<Vec<_>>();

        if values.len() != self.sections.len() {
            return Err(LineFormatError::MismatchedSectionCount {
                expected: self.sections.len(),
                actual: values.len(),
            });
        }

        let allocations = self.allocate_widths(values.as_slice(), available_width as usize)?;
        let mut rendered = Vec::with_capacity(self.sections.len());
        for ((section, value), width) in self
            .sections
            .iter()
            .zip(values.iter())
            .zip(allocations.into_iter())
        {
            rendered.push(FormattedLineSegment::new(
                render_section(value.as_str(), width, section.alignment, section.overflow),
                section.style,
            ));
        }

        Ok(rendered)
    }

    fn allocate_widths(
        &self,
        values: &[String],
        available_width: usize,
    ) -> Result<Vec<usize>, LineFormatError> {
        let mut widths = vec![0; self.sections.len()];
        let mut remaining = available_width;
        let mut flex_weight_total = 0usize;
        let mut flex_indices = Vec::new();
        let mut measured_indices = Vec::new();
        let mut fixed_indices = Vec::new();

        for (index, section) in self.sections.iter().enumerate() {
            match section.width {
                LineSectionWidth::Fixed(width) => {
                    widths[index] = usize::from(width);
                    fixed_indices.push(index);
                }
                LineSectionWidth::Measured => {
                    let width = display_width(values[index].as_str());
                    widths[index] = width;
                    measured_indices.push(index);
                }
                LineSectionWidth::Flex(weight) => {
                    let weight = usize::from(weight);
                    if weight == 0 {
                        return Err(LineFormatError::InvalidFlexWeight { section: index });
                    }
                    flex_weight_total += weight;
                    flex_indices.push((index, weight));
                }
            }
        }

        let mut non_flex_width = fixed_indices
            .iter()
            .chain(measured_indices.iter())
            .map(|index| widths[*index])
            .sum::<usize>();

        if non_flex_width > remaining {
            let mut overflow = non_flex_width - remaining;
            for index in measured_indices.iter().copied() {
                if overflow == 0 {
                    break;
                }
                let shrink = widths[index].min(overflow);
                widths[index] -= shrink;
                overflow -= shrink;
                non_flex_width -= shrink;
            }

            for index in fixed_indices.iter().copied() {
                if overflow == 0 {
                    break;
                }
                let shrink = widths[index].min(overflow);
                widths[index] -= shrink;
                overflow -= shrink;
                non_flex_width -= shrink;
            }
        }

        remaining = remaining.saturating_sub(non_flex_width);
        if flex_weight_total == 0 || remaining == 0 {
            return Ok(widths);
        }

        let mut flex_remaining = remaining;
        let mut weight_remaining = flex_weight_total;
        for (position, (index, weight)) in flex_indices.iter().enumerate() {
            let width = if position + 1 == flex_indices.len() {
                flex_remaining
            } else {
                (flex_remaining * *weight) / weight_remaining
            };
            widths[*index] = width;
            flex_remaining -= width;
            weight_remaining -= *weight;
        }

        Ok(widths)
    }
}

fn render_section(
    text: &str,
    width: usize,
    alignment: LineSectionAlignment,
    overflow: LineSectionOverflow,
) -> String {
    let text_width = display_width(text);
    if width == 0 {
        return String::new();
    }

    if text_width > width {
        return match overflow {
            LineSectionOverflow::Clip => clip_section_text(text, width, alignment),
            LineSectionOverflow::Ellipsis(placement) => {
                ellipsize_section_text(text, width, placement)
            }
        };
    }

    let padding = width - text_width;
    match alignment {
        LineSectionAlignment::Left => format!("{}{}", text, " ".repeat(padding)),
        LineSectionAlignment::Center => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
        LineSectionAlignment::Right => format!("{}{}", " ".repeat(padding), text),
    }
}

fn clip_section_text(text: &str, width: usize, alignment: LineSectionAlignment) -> String {
    let side = match alignment {
        LineSectionAlignment::Left => ClipSide::Start,
        LineSectionAlignment::Right => ClipSide::End,
        LineSectionAlignment::Center => ClipSide::Center,
    };
    clip_text(text, width, side).text
}

fn ellipsize_section_text(text: &str, width: usize, placement: EllipsisPlacement) -> String {
    let side = match placement {
        EllipsisPlacement::Start => EllipsisSide::Start,
        EllipsisPlacement::Middle => EllipsisSide::Middle,
        EllipsisPlacement::End => EllipsisSide::End,
    };
    ellipsize_text(text, width, side).text
}

#[cfg(test)]
mod tests {
    use super::*;
    use urvim_terminal::Color;

    fn segment_text(segments: &[FormattedLineSegment]) -> String {
        segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    #[test]
    fn template_allocates_fixed_and_flex_widths() {
        let template = FormattedLineTemplate::new(vec![
            FormattedLineSection::fixed(4, Style::new().fg(Color::ansi(1))),
            FormattedLineSection::flex(1, Style::new().fg(Color::ansi(2))),
            FormattedLineSection::flex(2, Style::new().fg(Color::ansi(3))),
        ]);

        let segments = template
            .render_segments(["ab", "cd", "efgh"], 16)
            .expect("rendered template");

        assert_eq!(segments.len(), 3);
        assert_eq!(segment_text(&segments), "ab  cd  efgh    ");
        assert_eq!(display_width(segments[0].text.as_str()), 4);
        assert_eq!(display_width(segments[1].text.as_str()), 4);
        assert_eq!(display_width(segments[2].text.as_str()), 8);
    }

    #[test]
    fn template_applies_alignment_within_allocated_width() {
        let template = FormattedLineTemplate::new(vec![
            FormattedLineSection::fixed(6, Style::default())
                .with_alignment(LineSectionAlignment::Left),
            FormattedLineSection::fixed(6, Style::default())
                .with_alignment(LineSectionAlignment::Center),
            FormattedLineSection::fixed(6, Style::default())
                .with_alignment(LineSectionAlignment::Right),
        ]);

        let segments = template
            .render_segments(["ab", "ab", "ab"], 18)
            .expect("rendered template");

        assert_eq!(segments[0].text, "ab    ");
        assert_eq!(segments[1].text, "  ab  ");
        assert_eq!(segments[2].text, "    ab");
    }

    #[test]
    fn template_applies_clip_and_ellipsis_overflow() {
        let template = FormattedLineTemplate::new(vec![
            FormattedLineSection::fixed(4, Style::default())
                .with_overflow(LineSectionOverflow::Clip),
            FormattedLineSection::fixed(4, Style::default())
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::End)),
            FormattedLineSection::fixed(4, Style::default())
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
            FormattedLineSection::fixed(4, Style::default())
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Middle)),
        ]);

        let segments = template
            .render_segments(["abcdef", "abcdef", "abcdef", "abcdef"], 16)
            .expect("rendered template");

        assert_eq!(segments[0].text, "abcd");
        assert_eq!(segments[1].text, "abc…");
        assert_eq!(segments[2].text, "…def");
        assert_eq!(segments[3].text, "ab…f");
    }

    #[test]
    fn template_measures_content_width_sections() {
        let template = FormattedLineTemplate::new(vec![
            FormattedLineSection::measured(Style::default()),
            FormattedLineSection::flex(1, Style::default())
                .with_alignment(LineSectionAlignment::Right)
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
            FormattedLineSection::measured(Style::default()),
        ]);

        let segments = template
            .render_segments(["ab", "long/path.rs", ":8:4"], 20)
            .expect("rendered template");

        assert_eq!(segments[0].text, "ab");
        assert!(segments[1].text.ends_with("path.rs"));
        assert_eq!(segments[2].text, ":8:4");
    }

    #[test]
    fn template_rejects_value_count_mismatch() {
        let template =
            FormattedLineTemplate::new(vec![FormattedLineSection::fixed(4, Style::default())]);

        let error = template
            .render_segments(std::iter::empty::<&str>(), 4)
            .expect_err("expected mismatch error");

        assert_eq!(
            error,
            LineFormatError::MismatchedSectionCount {
                expected: 1,
                actual: 0,
            }
        );
    }
}
