//! MD050: strong marker (`**text**` vs. `__text__`) should be consistent across the document.
//! `[rules.strong_style] style` accepts `"consistent"` (default), `"asterisk"`, or
//! `"underscore"`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct StrongStyle;

fn configured_marker(style: &str) -> Option<&'static str> {
    match style {
        "asterisk" => Some("**"),
        "underscore" => Some("__"),
        _ => None,
    }
}

impl Rule for StrongStyle {
    fn id(&self) -> RuleId {
        RuleId::StrongStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let mut expected = config
            .rule_options(self.id())
            .get_str("style")
            .and_then(configured_marker);
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Strong(_) = node else { return };
            let Some(position) = node.position() else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(raw) = crate::fix::slice(source, position.clone().into()) else {
                return;
            };
            if raw.len() < 4 {
                return;
            }
            let found = &raw[..2];
            if (found != "**" && found != "__") || &raw[raw.len() - 2..] != found {
                return;
            }
            let expected_marker = *expected.get_or_insert(found);

            if found != expected_marker {
                let inner = &raw[2..raw.len() - 2];
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::StrongStyle {
                            expected: expected_marker.to_string(),
                            found: found.to_string(),
                        },
                        self.default_severity(),
                    )
                    .with_range(position.clone())
                    .with_fix(Fix::new(
                        position.into(),
                        format!("{expected_marker}{inner}{expected_marker}"),
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
        StrongStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_consistent_asterisks() {
        assert!(run("**one** and **two**\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_marker() {
        let diagnostics = run("**one** and __two__\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "**two**");
    }
}
