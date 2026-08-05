//! Textual auto-fixes for lint diagnostics, applied by the CLI's `--fix` flag.
//!
//! Unlike `mq-lint` (whose rules see only the HIR, not raw source), rules here already receive
//! the raw source text in [`crate::rules::Rule::check`], so a [`Fix`] can just carry the final
//! replacement string computed at check time instead of a deferred, source-resolved expression.

use crate::Range;

/// A machine-applicable rewrite: replace the source spanned by `range` with `replacement`. A
/// zero-width `range` (see [`Range::at`]) is a pure insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub range: Range,
    pub replacement: String,
}

impl Fix {
    pub fn new(range: Range, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }
}

/// Converts a 1-based line/column position (column counted in `char`s) into a byte offset into
/// `source`.
fn position_to_byte_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    let mut offset = 0;
    let mut lines = source.split_inclusive('\n');

    for _ in 1..line {
        offset += lines.next()?.len();
    }
    let line_text = lines.next().unwrap_or("");

    let mut col = 1;
    for (i, _) in line_text.char_indices() {
        if col == column {
            return Some(offset + i);
        }
        col += 1;
    }
    if col == column {
        return Some(offset + line_text.trim_end_matches('\n').len());
    }

    None
}

fn range_to_byte_span(source: &str, range: Range) -> Option<(usize, usize)> {
    let start = position_to_byte_offset(source, range.start_line, range.start_column)?;
    let end = position_to_byte_offset(source, range.end_line, range.end_column)?;
    (start <= end).then_some((start, end))
}

/// Extracts the substring of `source` spanned by `range`. Used by rules that need to inspect a
/// node's exact raw syntax (e.g. distinguishing `[text][]` from `[text]`), not just its parsed
/// value.
pub(crate) fn slice(source: &str, range: Range) -> Option<&str> {
    let (start, end) = range_to_byte_span(source, range)?;
    source.get(start..end)
}

/// Applies `fixes` to `source`, returning the rewritten text.
///
/// Edits are applied from the end of the source towards the start so earlier byte offsets stay
/// valid; if two edits overlap, the one starting later wins (it's applied first, since we work
/// backwards) and any earlier edit whose span reaches into the already-applied region is dropped
/// — the same policy as `mq-lint`, which in practice mostly matters for same-start overlaps
/// (where the first one supplied wins, since the sort is stable).
pub fn apply_fixes(source: &str, fixes: &[Fix]) -> String {
    let mut spans: Vec<(usize, usize, &str)> = fixes
        .iter()
        .filter_map(|fix| {
            range_to_byte_span(source, fix.range).map(|(start, end)| (start, end, fix.replacement.as_str()))
        })
        .collect();
    spans.sort_by_key(|(start, ..)| std::cmp::Reverse(*start));

    let mut result = source.to_string();
    let mut applied_start = usize::MAX;
    for (start, end, text) in spans {
        if end > applied_start {
            continue;
        }
        result.replace_range(start..end, text);
        applied_start = start;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_fixes_replaces_a_single_span() {
        let source = "#Title\n";
        let fixes = vec![Fix::new(Range::single_line(1, 1, 2), "# ")];
        assert_eq!(apply_fixes(source, &fixes), "# Title\n");
    }

    #[test]
    fn apply_fixes_handles_multiple_non_overlapping_edits() {
        let source = "#One\n\n##Two\n";
        let fixes = vec![
            Fix::new(Range::single_line(1, 1, 2), "# "),
            Fix::new(Range::single_line(3, 1, 3), "## "),
        ];
        assert_eq!(apply_fixes(source, &fixes), "# One\n\n## Two\n");
    }

    #[test]
    fn apply_fixes_supports_pure_insertion_at_a_zero_width_range() {
        let source = "# Title\nBody\n";
        let fixes = vec![Fix::new(Range::at(2, 1), "\n")];
        assert_eq!(apply_fixes(source, &fixes), "# Title\n\nBody\n");
    }

    #[test]
    fn apply_fixes_drops_the_earlier_overlapping_edit() {
        let source = "hello world\n";
        let fixes = vec![
            Fix::new(Range::single_line(1, 1, 12), "replaced"),
            Fix::new(Range::single_line(1, 7, 12), "WORLD"),
        ];
        // The later-starting edit (WORLD, replacing just "world") is applied first; the
        // earlier, larger edit overlaps it and is dropped.
        assert_eq!(apply_fixes(source, &fixes), "hello WORLD\n");
    }
}
