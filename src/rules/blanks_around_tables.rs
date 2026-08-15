//! MD058: a table (the whole block, not each row individually) should be surrounded by a blank
//! line on each side.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct BlanksAroundTables;

fn is_table_node(node: &Node) -> bool {
    matches!(node, Node::TableCell(_) | Node::TableAlign(_) | Node::TableRow(_))
}

impl Rule for BlanksAroundTables {
    fn id(&self) -> RuleId {
        RuleId::BlanksAroundTables
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        let mut i = 0;
        while i < doc.nodes.len() {
            if !is_table_node(&doc.nodes[i]) {
                i += 1;
                continue;
            }
            let block_start = i;
            let mut j = i;
            while doc.nodes.get(j).is_some_and(is_table_node) {
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
                    Diagnostic::new(LintMessage::BlanksAroundTables { above: true }, self.default_severity())
                        .with_range(Range::at(start, 1))
                        .with_fix(Fix::new(Range::at(start, 1), "\n")),
                );
            }
            if let Some(end) = end_line
                && end < lines.len()
                && lines.get(end).is_some_and(|l| !l.trim().is_empty())
            {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::BlanksAroundTables { above: false },
                        self.default_severity(),
                    )
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
        BlanksAroundTables.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_properly_surrounded() {
        assert!(run("Intro\n\n| A |\n|---|\n| 1 |\n\nBody\n").is_empty());
    }

    #[test]
    fn flags_missing_blank_lines_around_the_table() {
        // A heading unambiguously ends the table (unlike plain text, which GFM treats as
        // another lazily-continued table row).
        let diagnostics = run("Intro\n| A |\n|---|\n| 1 |\n# Heading\n");
        assert_eq!(diagnostics.len(), 2);
    }
}
