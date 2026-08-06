//! MD025: a document should have only one top-level (`h1`) heading. Flags every `h1` after the
//! first. Not auto-fixable — whether to demote it or remove the earlier one is an editorial call.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct SingleH1;

impl Rule for SingleH1 {
    fn id(&self) -> RuleId {
        RuleId::SingleH1
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut h1s: Vec<Option<mq_markdown::Position>> = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Heading(heading) = node
                && heading.depth == 1
            {
                h1s.push(heading.position.clone());
            }
        });
        h1s.sort_by_key(|pos| pos.as_ref().map(|p| (p.start.line, p.start.column)));

        h1s.into_iter()
            .skip(1)
            .map(|position| {
                let mut diagnostic = Diagnostic::new(LintMessage::SingleH1, self.default_severity());
                if let Some(position) = position {
                    diagnostic = diagnostic.with_range(position);
                }
                diagnostic
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        SingleH1.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_single_h1() {
        assert!(run("# Title\n\n## Section\n").is_empty());
    }

    #[test]
    fn flags_every_h1_after_the_first() {
        assert_eq!(run("# One\n\n# Two\n\n# Three\n").len(), 2);
    }
}
