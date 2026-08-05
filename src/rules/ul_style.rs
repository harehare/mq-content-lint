//! MD004: unordered list marker (`-`, `*`, or `+`) should be consistent across the document.
//! `[rules.ul_style] style` accepts `"consistent"` (default — match the first list item found),
//! `"dash"`, `"asterisk"`, or `"plus"`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct UlStyle;

fn style_char(style: &str) -> Option<char> {
    match style {
        "dash" => Some('-'),
        "asterisk" => Some('*'),
        "plus" => Some('+'),
        _ => None,
    }
}

impl Rule for UlStyle {
    fn id(&self) -> RuleId {
        RuleId::UlStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let mut expected = options.get_str("style").and_then(style_char);

        let mut diagnostics = Vec::new();
        for node in &doc.nodes {
            let Node::List(list) = node else { continue };
            if list.ordered {
                continue;
            }
            let Some(position) = &list.position else { continue };
            let Some((_, line)) = crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line) else {
                continue;
            };
            let Some(marker) = line.trim_start().chars().next().filter(|c| "-*+".contains(*c)) else {
                continue;
            };
            let expected_char = *expected.get_or_insert(marker);

            if marker != expected_char {
                let indent = line.len() - line.trim_start().len();
                let column = indent + 1;
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::UlStyle {
                            expected: expected_char,
                            found: marker,
                        },
                        self.default_severity(),
                    )
                    .with_range(Range::single_line(position.start.line, column, column + 1))
                    .with_fix(Fix::new(
                        Range::single_line(position.start.line, column, column + 1),
                        expected_char.to_string(),
                    )),
                );
            }
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        UlStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_consistent_marker() {
        assert!(run("- one\n- two\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_marker() {
        let diagnostics = run("- one\n* two\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "-");
    }

    #[test]
    fn respects_configured_style() {
        let config = LintConfig::from_toml_str("[rules.ul_style]\nstyle = \"plus\"\n").unwrap();
        let doc: mq_markdown::Markdown = "- one\n".parse().unwrap();
        let diagnostics = UlStyle.check(&doc, "- one\n", &config);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "+");
    }
}
