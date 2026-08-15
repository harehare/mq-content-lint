//! MD005: list items at the same nesting level should be indented consistently with each
//! other. Tracks the first indentation seen at each level within a contiguous run of list
//! items; a non-list block resets tracking, so unrelated lists elsewhere in the document don't
//! have to agree with each other.
//!
//! Uses the raw line's leading whitespace, not `Position::start.column` — a list item's position
//! in `mq-markdown` points at the start of its *content* (after the marker and its trailing
//! space), not the start of the marker itself.

use std::collections::HashMap;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct ListIndent;

impl Rule for ListIndent {
    fn id(&self) -> RuleId {
        RuleId::ListIndent
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
        let mut expected_by_level: HashMap<u8, usize> = HashMap::new();
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        for node in &doc.nodes {
            let Node::List(list) = node else {
                expected_by_level.clear();
                continue;
            };
            let Some(position) = &list.position else { continue };
            let Some(line) = lines.get(position.start.line) else {
                continue;
            };
            let indent = line.len() - line.trim_start().len();
            let expected = *expected_by_level.entry(list.level).or_insert(indent);

            if indent != expected {
                let indent_range = Range::single_line(position.start.line, 1, indent + 1);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::ListIndent {
                            expected,
                            found: indent,
                        },
                        self.default_severity(),
                    )
                    .with_range(indent_range)
                    .with_fix(Fix::new(indent_range, " ".repeat(expected))),
                );
            }
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        ListIndent.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_consistent_indentation() {
        assert!(run("- one\n- two\n").is_empty());
    }

    #[test]
    fn flags_inconsistent_sibling_indentation() {
        let diagnostics = run("- one\n  - nested\n   - also nested but off by one\n");
        assert_eq!(diagnostics.len(), 1);
    }
}
