//! MD042: a link with no real destination — an empty URL or a bare `#` placeholder. Not
//! auto-fixable — there's no way to invent the intended destination.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct NoEmptyLinks;

impl Rule for NoEmptyLinks {
    fn id(&self) -> RuleId {
        RuleId::NoEmptyLinks
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        _source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Link(link) = node else { return };
            let url = link.url.as_str();
            if url.is_empty() || url == "#" {
                let mut diagnostic = Diagnostic::new(LintMessage::NoEmptyLinks, self.default_severity());
                if let Some(position) = link.position.clone() {
                    diagnostic = diagnostic.with_range(position);
                }
                diagnostics.push(diagnostic);
            }
        });
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        NoEmptyLinks.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_a_real_link() {
        assert!(run("[text](https://example.com)\n").is_empty());
    }

    #[test]
    fn flags_an_empty_url() {
        assert_eq!(run("[text]()\n").len(), 1);
    }

    #[test]
    fn flags_a_hash_placeholder() {
        assert_eq!(run("[text](#)\n").len(), 1);
    }
}
