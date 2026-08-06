//! MD049: emphasis marker (`*text*` vs. `_text_`) should be consistent across the document.
//! `[rules.emphasis_style] style` accepts `"consistent"` (default), `"asterisk"`, or
//! `"underscore"`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct EmphasisStyle;

fn configured_char(style: &str) -> Option<char> {
    match style {
        "asterisk" => Some('*'),
        "underscore" => Some('_'),
        _ => None,
    }
}

impl Rule for EmphasisStyle {
    fn id(&self) -> RuleId {
        RuleId::EmphasisStyle
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
            let Node::Emphasis(_) = node else { return };
            let Some(position) = node.position() else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(raw) = crate::fix::slice(source, position.clone().into()) else {
                return;
            };
            let chars: Vec<char> = raw.chars().collect();
            if chars.len() < 3 {
                return;
            }
            let found = chars[0];
            if (found != '*' && found != '_') || chars[chars.len() - 1] != found {
                return;
            }
            let expected_char = *expected.get_or_insert(found);

            if found != expected_char {
                let inner: String = chars[1..chars.len() - 1].iter().collect();
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::EmphasisStyle {
                            expected: expected_char,
                            found,
                        },
                        self.default_severity(),
                    )
                    .with_range(position.clone())
                    .with_fix(Fix::new(
                        position.into(),
                        format!("{expected_char}{inner}{expected_char}"),
                    )),
                );
            }
        });
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["style"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        EmphasisStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_consistent_asterisks() {
        assert!(run("*one* and *two*\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_marker() {
        let diagnostics = run("*one* and _two_\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "*two*");
    }
}
