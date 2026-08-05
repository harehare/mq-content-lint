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
}
