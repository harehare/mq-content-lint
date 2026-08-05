//! MD046: code block style (fenced vs. indented) should be consistent across the document.
//! `[rules.code_block_style] style` accepts `"consistent"` (default — match the first code
//! block found), `"fenced"`, or `"indented"`. Not auto-fixable — converting between the two
//! changes the block's raw form significantly enough that this rule only flags it.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct CodeBlockStyle;

impl Rule for CodeBlockStyle {
    fn id(&self) -> RuleId {
        RuleId::CodeBlockStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let configured = config.rule_options(self.id()).get_str("style").map(str::to_string);
        let mut expected: Option<bool> = configured.as_deref().map(|s| s == "fenced");

        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Code(code) = node else { return };
            let expected_fenced = *expected.get_or_insert(code.fence);
            if code.fence != expected_fenced {
                let mut diagnostic = Diagnostic::new(
                    LintMessage::CodeBlockStyle {
                        expected: if expected_fenced { "fenced" } else { "indented" }.to_string(),
                    },
                    self.default_severity(),
                );
                if let Some(position) = code.position.clone() {
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
        CodeBlockStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_all_fenced() {
        assert!(run("```\none\n```\n\n```\ntwo\n```\n").is_empty());
    }

    #[test]
    fn flags_a_mixed_style() {
        let diagnostics = run("```\nfenced\n```\n\n    indented\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::CodeBlockStyle {
                expected: "fenced".to_string()
            }
        );
    }
}
