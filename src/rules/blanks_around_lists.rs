//! MD032: a list (the whole contiguous block of items, not each item individually) should be
//! surrounded by a blank line on each side, except at the very start/end of the file.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct BlanksAroundLists;

impl Rule for BlanksAroundLists {
    fn id(&self) -> RuleId {
        RuleId::BlanksAroundLists
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        let mut i = 0;
        while i < doc.nodes.len() {
            if !matches!(doc.nodes[i], Node::List(_)) {
                i += 1;
                continue;
            }
            let block_start = i;
            let mut j = i;
            while matches!(doc.nodes.get(j), Some(Node::List(_))) {
                j += 1;
            }
            let block_end = j - 1;

            let start_line = doc.nodes[block_start].position().map(|p| p.start.line);
            let end_line = doc.nodes[block_end].position().map(|p| p.end.line);

            if let Some(start) = start_line
                && start > 1
                && lines.get(start - 2).is_some_and(|l| !l.trim().is_empty())
            {
                diagnostics.push(
                    Diagnostic::new(LintMessage::BlanksAroundLists { above: true }, self.default_severity())
                        .with_range(Range::at(start, 1))
                        .with_fix(Fix::new(Range::at(start, 1), "\n")),
                );
            }
            if let Some(end) = end_line
                && end < lines.len()
                && lines.get(end).is_some_and(|l| !l.trim().is_empty())
            {
                diagnostics.push(
                    Diagnostic::new(LintMessage::BlanksAroundLists { above: false }, self.default_severity())
                        .with_range(Range::at(end, lines[end - 1].chars().count() + 1))
                        .with_fix(Fix::new(Range::at(end + 1, 1), "\n")),
                );
            }

            i = j;
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        BlanksAroundLists.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_when_properly_surrounded() {
        assert!(run("Intro\n\n- one\n- two\n\nBody\n").is_empty());
    }

    #[test]
    fn flags_missing_blank_lines_around_the_whole_list() {
        // A heading unambiguously ends the list (unlike plain text, which CommonMark treats as
        // a lazy continuation of the last item's paragraph).
        let diagnostics = run("Intro\n- one\n- two\n# Heading\n");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_between_sibling_items() {
        assert!(run("\n- one\n- two\n- three\n").is_empty());
    }
}
