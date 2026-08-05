//! MD048: fenced code block fence character (backtick ` ``` ` vs. tilde `~~~`) should be
//! consistent across the document. `[rules.code_fence_style] style` accepts `"consistent"`
//! (default), `"backtick"`, or `"tilde"`. Not auto-fixable — the opening and closing fence must
//! change together, which this crate's single-range `Fix` can't express in one diagnostic.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct CodeFenceStyle;

fn configured_char(style: &str) -> Option<char> {
    match style {
        "backtick" => Some('`'),
        "tilde" => Some('~'),
        _ => None,
    }
}

impl Rule for CodeFenceStyle {
    fn id(&self) -> RuleId {
        RuleId::CodeFenceStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let mut expected = config
            .rule_options(self.id())
            .get_str("style")
            .and_then(configured_char);
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Code(code) = node else { return };
            if !code.fence {
                return;
            }
            let Some(position) = &code.position else { return };
            let Some((_, line)) = crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line) else {
                return;
            };
            let Some(found) = line.trim_start().chars().next().filter(|c| *c == '`' || *c == '~') else {
                return;
            };
            let expected_char = *expected.get_or_insert(found);

            if found != expected_char {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::CodeFenceStyle {
                            expected: expected_char,
                        },
                        self.default_severity(),
                    )
                    .with_range(position.clone()),
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
        CodeFenceStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_consistent_backticks() {
        assert!(run("```\none\n```\n\n```\ntwo\n```\n").is_empty());
    }

    #[test]
    fn flags_an_inconsistent_fence_character() {
        let diagnostics = run("```\none\n```\n\n~~~\ntwo\n~~~\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, LintMessage::CodeFenceStyle { expected: '`' });
    }
}
