//! Small helpers for rules that need to inspect raw source lines rather than the AST.

/// Returns `source`'s lines paired with their 1-based line number, matching
/// `mq_markdown::Position`'s line numbering.
pub(crate) fn numbered_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source.lines().enumerate().map(|(i, line)| (i + 1, line))
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
