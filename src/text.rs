//! Small helpers for rules that need to inspect raw source lines rather than the AST.

/// Returns `source`'s lines paired with their 1-based line number, matching
/// `mq_markdown::Position`'s line numbering.
pub(crate) fn numbered_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source.lines().enumerate().map(|(i, line)| (i + 1, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_lines_starts_at_one() {
        let lines: Vec<_> = numbered_lines("a\nb\nc").collect();
        assert_eq!(lines, vec![(1, "a"), (2, "b"), (3, "c")]);
    }
}
