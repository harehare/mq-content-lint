//! MD039: spaces just inside a link's text brackets (`[ text ](url)` instead of `[text](url)`).

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoSpaceInLinks;

impl Rule for NoSpaceInLinks {
    fn id(&self) -> RuleId {
        RuleId::NoSpaceInLinks
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let byte_index = crate::text::LineByteIndex::new(source);
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Link(link) = node else { return };
            let Some(position) = &link.position else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(raw) = crate::fix::slice(source, &byte_index, position.clone().into()) else {
                return;
            };
            let Some(open) = raw.find('[') else { return };
            let Some(close) = raw[open..].find(']').map(|i| open + i) else {
                return;
            };
            let inner = &raw[open + 1..close];
            let trimmed = inner.trim();
            if inner != trimmed && !inner.trim().is_empty() {
                let start_col = position.start.column + raw[..open + 1].chars().count();
                let end_col = position.start.column + raw[..close].chars().count();
                let range = Range::single_line(position.start.line, start_col, end_col);
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoSpaceInLinks, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, trimmed.to_string())),
                );
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
        NoSpaceInLinks.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_clean_link_text() {
        assert!(run("[text](https://example.com)\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_spaces_inside_brackets() {
        let diagnostics = run("[ text ](https://example.com)\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "text");
    }
}
