//! Flags a heading whose depth jumps by more than one level from the previous heading, e.g. an
//! `h1` followed directly by an `h3` with no `h2` in between. Equivalent to markdownlint's MD001.
//!
//! Decreasing depth (`h3` back to `h1`) is always fine — that's just closing out a subsection —
//! and the very first heading in a document is never flagged regardless of its level (whether a
//! document must start at `h1` is a different, narrower rule this crate doesn't implement).

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct HeadingHierarchySkip;

impl Rule for HeadingHierarchySkip {
    fn id(&self) -> RuleId {
        RuleId::HeadingHierarchySkip
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut headings: Vec<(u8, Option<mq_markdown::Position>)> = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Heading(heading) = node {
                headings.push((heading.depth, heading.position.clone()));
            }
        });
        // Headings found while descending into a container (e.g. one nested in a blockquote)
        // aren't guaranteed to come out in document order, so sort explicitly.
        headings.sort_by_key(|(_, pos)| pos.as_ref().map(|p| (p.start.line, p.start.column)));

        let mut diagnostics = Vec::new();
        let mut previous_depth: Option<u8> = None;

        for (depth, position) in headings {
            if let Some(prev) = previous_depth
                && depth > prev + 1
            {
                let mut diagnostic = Diagnostic::new(
                    LintMessage::HeadingHierarchySkip { from: prev, to: depth },
                    self.default_severity(),
                );
                if let Some(position) = position.clone() {
                    diagnostic = diagnostic.with_range(position);
                }
                diagnostics.push(diagnostic);
            }
            previous_depth = Some(depth);
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        HeadingHierarchySkip.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_sequential_headings() {
        assert!(run("# One\n\n## Two\n\n### Three\n").is_empty());
    }

    #[test]
    fn no_diagnostics_for_decreasing_depth() {
        assert!(run("# One\n\n## Two\n\n# Three\n").is_empty());
    }

    #[test]
    fn no_diagnostics_when_document_starts_below_h1() {
        assert!(run("## Two\n\n### Three\n").is_empty());
    }

    #[test]
    fn flags_a_skipped_level() {
        let diagnostics = run("# One\n\n### Three\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::HeadingHierarchySkip { from: 1, to: 3 }
        );
        assert_eq!(diagnostics[0].range.unwrap().start_line, 3);
    }

    #[test]
    fn flags_each_skip_independently() {
        let diagnostics = run("# One\n\n### Three\n\n##### Five\n");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[1].message,
            LintMessage::HeadingHierarchySkip { from: 3, to: 5 }
        );
    }

    #[test]
    fn finds_headings_nested_inside_a_blockquote() {
        let diagnostics = run("# One\n\n> ### Nested\n");
        assert_eq!(diagnostics.len(), 1);
    }
}
