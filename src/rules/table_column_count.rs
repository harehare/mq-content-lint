//! MD056: every row in a table should have the same number of cells as the header row. Not
//! auto-fixable — inserting or deleting a cell to fix the count needs to know which column was
//! intended, which this rule can't determine.

use std::collections::BTreeMap;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct TableColumnCount;

impl Rule for TableColumnCount {
    fn id(&self) -> RuleId {
        RuleId::TableColumnCount
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        // Group consecutive table cells into per-table blocks, since a document can contain
        // more than one table and each restarts its own row numbering at 0.
        let mut diagnostics = Vec::new();
        let mut current: BTreeMap<usize, (usize, Option<mq_markdown::Position>)> = BTreeMap::new();
        let mut in_table = false;

        let flush = |rows: &BTreeMap<usize, (usize, Option<mq_markdown::Position>)>,
                     diagnostics: &mut Vec<Diagnostic>| {
            let Some((_, (header_count, _))) = rows.iter().next() else {
                return;
            };
            let header_count = *header_count;
            for (_, (count, position)) in rows.iter().skip(1) {
                if *count != header_count {
                    let mut diagnostic = Diagnostic::new(
                        LintMessage::TableColumnCount {
                            expected: header_count,
                            found: *count,
                        },
                        Severity::Warning,
                    );
                    if let Some(position) = position {
                        diagnostic = diagnostic.with_range(position.clone());
                    }
                    diagnostics.push(diagnostic);
                }
            }
        };

        for node in &doc.nodes {
            if let Node::TableCell(cell) = node {
                in_table = true;
                let entry = current.entry(cell.row).or_insert((0, cell.position.clone()));
                entry.0 += 1;
            } else if in_table && !matches!(node, Node::TableAlign(_)) {
                flush(&current, &mut diagnostics);
                current.clear();
                in_table = false;
            }
        }
        if in_table {
            flush(&current, &mut diagnostics);
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        TableColumnCount.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_regular_table() {
        assert!(run("| A | B |\n|---|---|\n| 1 | 2 |\n").is_empty());
    }

    #[test]
    fn flags_a_row_with_too_few_cells() {
        let diagnostics = run("| A | B | C |\n|---|---|---|\n| 1 | 2 |\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::TableColumnCount { expected: 3, found: 2 }
        );
    }
}
