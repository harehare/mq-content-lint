//! MD059: link text should describe the destination, not be a generic phrase like "click here".
//! Configurable forbidden phrase list via `[rules.descriptive_link_text] forbidden` (matched
//! case-insensitively against the link's full trimmed text). Not auto-fixable — writing
//! descriptive text needs to know what the destination actually is.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct DescriptiveLinkText;

const DEFAULT_FORBIDDEN: &[&str] = &["click here", "here", "link", "this", "more", "read more", "click"];

impl Rule for DescriptiveLinkText {
    fn id(&self) -> RuleId {
        RuleId::DescriptiveLinkText
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let forbidden = options.get_str_array("forbidden");
        let forbidden: Vec<String> = forbidden
            .unwrap_or_else(|| DEFAULT_FORBIDDEN.iter().map(|s| s.to_string()).collect())
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect();

        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Link(link) = node else { return };
            let link_text: String = link.values.iter().map(|v| v.to_string()).collect();
            let text = link_text.trim().to_lowercase();
            if forbidden.contains(&text) {
                let mut diagnostic = Diagnostic::new(
                    LintMessage::DescriptiveLinkText {
                        text: link_text.clone(),
                    },
                    self.default_severity(),
                );
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
        DescriptiveLinkText.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_descriptive_text() {
        assert!(run("[the pricing page](https://example.com/pricing)\n").is_empty());
    }

    #[test]
    fn flags_click_here() {
        assert_eq!(run("[Click here](https://example.com)\n").len(), 1);
    }
}
