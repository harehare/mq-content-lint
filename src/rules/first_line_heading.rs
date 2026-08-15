//! MD041: the document should start with a top-level (`h1`) heading — the first content node
//! after any front matter block. Not auto-fixable — there's no way to invent a title.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct FirstLineHeading;

impl Rule for FirstLineHeading {
    fn id(&self) -> RuleId {
        RuleId::FirstLineHeading
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
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
        let first_content = doc.nodes.iter().find(|n| !matches!(n, Node::Yaml(_) | Node::Toml(_)));

        let is_h1 = matches!(first_content, Some(Node::Heading(h)) if h.depth == 1);
        if is_h1 || first_content.is_none() {
            return Vec::new();
        }

        let mut diagnostic = Diagnostic::new(LintMessage::FirstLineHeading, self.default_severity());
        if let Some(position) = first_content.and_then(|n| n.position()) {
            diagnostic = diagnostic.with_range(position);
        }
        vec![diagnostic]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        FirstLineHeading.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_document_starts_with_h1() {
        assert!(run("# Title\n\nBody\n").is_empty());
    }

    #[test]
    fn front_matter_is_skipped_before_checking() {
        assert!(run("---\ntitle: Hello\n---\n\n# Title\n").is_empty());
    }

    #[test]
    fn flags_a_document_that_does_not_start_with_h1() {
        let diagnostics = run("Just a paragraph.\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_document_starting_with_h2() {
        assert_eq!(run("## Not top level\n").len(), 1);
    }
}
