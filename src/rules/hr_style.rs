//! MD035: horizontal rule style (`---`, `***`, `___`, ...) should be consistent across the
//! document. `[rules.hr_style] style` accepts `"consistent"` (default — match the first rule
//! found) or an explicit literal like `"---"`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct HrStyle;

impl Rule for HrStyle {
    fn id(&self) -> RuleId {
        RuleId::HrStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let mut expected = config.rule_options(self.id()).get_str("style").map(str::to_string);
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::HorizontalRule(_) = node else { return };
            let Some(position) = node.position() else { return };
            let Some(line) = lines.get(position.start.line) else {
                return;
            };
            let found = line.trim().to_string();
            let expected_style = expected.get_or_insert_with(|| found.clone()).clone();

            if found != expected_style {
                let range = Range::single_line(position.start.line, 1, line.chars().count() + 1);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::HrStyle {
                            expected: expected_style.clone(),
                            found,
                        },
                        self.default_severity(),
                    )
                    .with_range(range)
                    .with_fix(Fix::new(range, expected_style)),
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
        HrStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_consistent_style() {
        assert!(run("---\n\ntext\n\n---\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_style() {
        let diagnostics = run("---\n\ntext\n\n***\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "---");
    }
}
