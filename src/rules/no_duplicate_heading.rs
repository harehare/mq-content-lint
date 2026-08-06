//! MD024: multiple headings in the document with identical text. Not auto-fixable — there's no
//! way to invent distinct wording for the duplicate.

use std::collections::HashSet;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct NoDuplicateHeading;

impl Rule for NoDuplicateHeading {
    fn id(&self) -> RuleId {
        RuleId::NoDuplicateHeading
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut headings: Vec<(String, Option<mq_markdown::Position>)> = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Heading(heading) = node {
                headings.push((node.value(), heading.position.clone()));
            }
        });
        headings.sort_by_key(|(_, pos)| pos.as_ref().map(|p| (p.start.line, p.start.column)));

        let mut seen = HashSet::new();
        let mut diagnostics = Vec::new();
        for (text, position) in headings {
            if !seen.insert(text.clone()) {
                let mut diagnostic = Diagnostic::new(LintMessage::NoDuplicateHeading { text }, self.default_severity());
                if let Some(position) = position {
                    diagnostic = diagnostic.with_range(position);
                }
                diagnostics.push(diagnostic);
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
        NoDuplicateHeading.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_distinct_headings() {
        assert!(run("# One\n\n## Two\n").is_empty());
    }

    #[test]
    fn flags_a_duplicate_heading() {
        let diagnostics = run("# Overview\n\n## Details\n\n## Overview\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::NoDuplicateHeading {
                text: "Overview".to_string()
            }
        );
    }
}
