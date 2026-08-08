//! Small helpers for rules that need to inspect raw source lines rather than the AST.

/// Returns `source`'s lines paired with their 1-based line number, matching
/// `mq_markdown::Position`'s line numbering.
pub(crate) fn numbered_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source.lines().enumerate().map(|(i, line)| (i + 1, line))
}

/// A document's fenced/indented code block line ranges (`(start_line, end_line)`, 1-based and
/// inclusive), for O(log n) "is this line inside a code block" lookups.
///
/// Several rules scan raw source lines rather than the AST (so they can catch text that never
/// became the node type they're checking for) and skip lines inside code blocks while doing it —
/// checking membership by scanning the range list for every single line makes such a rule
/// O(lines × code blocks) overall, quadratic for a document where both scale with size. Build one
/// `CodeBlockLines` from the same ranges before the per-line loop instead.
pub(crate) struct CodeBlockLines {
    /// Sorted by `start_line`, non-overlapping — exactly what walking a document's `Code` nodes
    /// in document order produces, which is the only way this is ever constructed.
    ranges: Vec<(usize, usize)>,
}

impl CodeBlockLines {
    pub(crate) fn new(ranges: Vec<(usize, usize)>) -> Self {
        Self { ranges }
    }

    /// Whether `line_number` (1-based) falls inside any code block range.
    pub(crate) fn contains(&self, line_number: usize) -> bool {
        // Ranges are sorted and non-overlapping, so the only one that could contain
        // `line_number` is the last one starting at or before it.
        match self.ranges.partition_point(|&(start, _)| start <= line_number) {
            0 => false,
            i => line_number <= self.ranges[i - 1].1,
        }
    }
}

/// `source`'s lines, indexed once for O(1) lookup by 1-based line number.
///
/// A rule that needs a specific line for each of `N` matching AST nodes (e.g. every heading,
/// every list item) must not look it up via `numbered_lines(source).find(...)` — that rescans
/// from the start of the document every time, making the rule O(N × lines) overall. For a
/// document where matches scale with size (a long list, a long sequence of headings) that's
/// quadratic. Build one `LineIndex` before the node loop instead.
pub(crate) struct LineIndex<'a> {
    lines: Vec<&'a str>,
}

impl<'a> LineIndex<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
        }
    }

    /// The text of `line_number` (1-based), or `None` if it's out of range.
    pub(crate) fn get(&self, line_number: usize) -> Option<&'a str> {
        line_number.checked_sub(1).and_then(|i| self.lines.get(i)).copied()
    }
}

/// `source`'s lines, indexed once by byte offset for O(1) 1-based line/column-to-byte-offset
/// conversion (used by [`crate::fix::slice`]/[`crate::fix::apply_fixes`]).
///
/// Same motivation as [`LineIndex`]: converting a position to a byte offset by scanning from the
/// start of the document (summing line lengths) on every call makes a rule that does it once per
/// matching node O(N × lines) overall — quadratic for a document where matches scale with size.
/// Build one `LineByteIndex` before the node loop instead.
pub(crate) struct LineByteIndex<'a> {
    /// Each line's byte offset from the start of the source, paired with its text (no line
    /// ending), indexed by line number - 1.
    lines: Vec<(usize, &'a str)>,
    source_len: usize,
}

impl<'a> LineByteIndex<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        let mut lines = Vec::new();
        let mut offset = 0;
        for line in source.split_inclusive('\n') {
            let trimmed = line.strip_suffix('\n').unwrap_or(line);
            lines.push((offset, trimmed));
            offset += line.len();
        }
        Self {
            lines,
            source_len: source.len(),
        }
    }

    /// Converts a 1-based line/column position (column counted in `char`s) into a byte offset
    /// into the source this index was built from.
    ///
    /// `line` may also be exactly one past the last real line (with `column` 1) — several fixes
    /// (e.g. `single_trailing_newline`'s, `link_image_reference_definitions`'s) target a range
    /// ending at this one-past-the-end sentinel to mean "through the rest of the file" on a
    /// source with no trailing newline, where there's no real final line to anchor to.
    pub(crate) fn byte_offset(&self, line: usize, column: usize) -> Option<usize> {
        let index = line.checked_sub(1)?;
        if let Some(&(line_start, line_text)) = self.lines.get(index) {
            let mut col = 1;
            for (i, _) in line_text.char_indices() {
                if col == column {
                    return Some(line_start + i);
                }
                col += 1;
            }
            if col == column {
                return Some(line_start + line_text.len());
            }
            None
        } else if index == self.lines.len() && column == 1 {
            Some(self.source_len)
        } else {
            None
        }
    }

    /// Converts an `mq_markdown::Position`'s 1-based column — which counts UTF-8 *bytes*, not
    /// `char`s, unlike every other column this crate computes by scanning raw line text — into
    /// the char-counted column [`Self::byte_offset`] (and everything built on it: [`Range`],
    /// [`crate::fix::slice`]) expects. A rule that reads `position.start.column`/`.end.column`
    /// must convert through this before combining it with a char-counted offset or constructing a
    /// `Range` from it, or the two conventions silently misalign on any line with multi-byte
    /// characters — landing mid-character and panicking once something tries to slice there.
    pub(crate) fn char_column(&self, line: usize, byte_column: usize) -> Option<usize> {
        let index = line.checked_sub(1)?;
        let &(_, line_text) = self.lines.get(index)?;
        let byte_offset = byte_column.checked_sub(1)?;
        if byte_offset == line_text.len() {
            return Some(line_text.chars().count() + 1);
        }
        if byte_offset > line_text.len() || !line_text.is_char_boundary(byte_offset) {
            return None;
        }
        Some(line_text[..byte_offset].chars().count() + 1)
    }
}

#[cfg(test)]
mod line_byte_index_tests {
    use super::*;

    #[test]
    fn char_column_converts_byte_based_ast_columns_on_multi_byte_lines() {
        let source = "` 従うように `\n";
        let index = LineByteIndex::new(source);
        // Byte length of the line (no trailing newline) is 19, so mq_markdown's own byte-based
        // convention reports the code span's end column as 20 (one past the last byte) — see
        // `no_space_in_code`'s doc comment for how this was discovered.
        assert_eq!(index.char_column(1, 1), Some(1));
        assert_eq!(index.char_column(1, 20), Some(10));
    }

    #[test]
    fn char_column_matches_byte_offset_on_pure_ascii_lines() {
        let source = "hello world\n";
        let index = LineByteIndex::new(source);
        for column in 1..=12 {
            assert_eq!(index.char_column(1, column), Some(column));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_lines_starts_at_one() {
        let lines: Vec<_> = numbered_lines("a\nb\nc").collect();
        assert_eq!(lines, vec![(1, "a"), (2, "b"), (3, "c")]);
    }

    #[test]
    fn code_block_lines_reports_membership_in_a_range() {
        let code = CodeBlockLines::new(vec![(3, 5), (10, 10)]);
        assert!(!code.contains(1));
        assert!(!code.contains(2));
        assert!(code.contains(3));
        assert!(code.contains(4));
        assert!(code.contains(5));
        assert!(!code.contains(6));
        assert!(!code.contains(9));
        assert!(code.contains(10));
        assert!(!code.contains(11));
    }

    #[test]
    fn code_block_lines_with_no_ranges_contains_nothing() {
        let code = CodeBlockLines::new(Vec::new());
        assert!(!code.contains(1));
    }

    #[test]
    fn line_index_looks_up_one_based_lines() {
        let index = LineIndex::new("a\nb\nc");
        assert_eq!(index.get(1), Some("a"));
        assert_eq!(index.get(3), Some("c"));
        assert_eq!(index.get(0), None);
        assert_eq!(index.get(4), None);
    }

    #[test]
    fn line_byte_index_converts_positions_within_the_document() {
        let source = "ab\ncde\n";
        let index = LineByteIndex::new(source);
        assert_eq!(index.byte_offset(1, 1), Some(0));
        assert_eq!(index.byte_offset(1, 3), Some(2)); // one past "ab"
        assert_eq!(index.byte_offset(2, 1), Some(3));
        assert_eq!(index.byte_offset(2, 4), Some(6)); // one past "cde"
        assert_eq!(index.byte_offset(0, 1), None);
        assert_eq!(index.byte_offset(1, 10), None);
    }

    #[test]
    fn line_byte_index_accepts_one_line_past_the_end_at_column_one() {
        // Several fixes target a range ending one line past the last real line (column 1) to
        // mean "through the rest of a file with no trailing newline" — must resolve to the very
        // end of the source, matching the pre-LineByteIndex behavior.
        let index = LineByteIndex::new("Hello");
        assert_eq!(index.byte_offset(1, 6), Some(5)); // end of "Hello" itself
        assert_eq!(index.byte_offset(2, 1), Some(5)); // one past the only real line
        assert_eq!(index.byte_offset(2, 2), None); // not column 1: not the sentinel case
        assert_eq!(index.byte_offset(3, 1), None); // more than one line past the end
    }
}
