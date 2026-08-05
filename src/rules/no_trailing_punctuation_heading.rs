//! MD026: a heading ending in trailing punctuation (`.,;:!` by default — configurable via
//! `[rules.no_trailing_punctuation_heading] punctuation`). `?` is excluded by default since
//! question-style headings are common and intentional.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoTrailingPunctuationHeading;

const DEFAULT_PUNCTUATION: &str = ".,;:!";

impl Rule for NoTrailingPunctuationHeading {
    fn id(&self) -> RuleId {
        RuleId::NoTrailingPunctuationHeading
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let punctuation: Vec<char> = options
            .get_str("punctuation")
            .map(|s| s.chars().collect())
            .unwrap_or_else(|| DEFAULT_PUNCTUATION.chars().collect());

        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let Some((_, line)) = crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line) else {
                return;
            };
            let trimmed = line.trim_end();
            let Some(last) = trimmed.chars().next_back() else {
                return;
            };
            if punctuation.contains(&last) {
                let column = trimmed.chars().count();
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::NoTrailingPunctuationHeading { punctuation: last },
                        self.default_severity(),
                    )
                    .with_range(Range::single_line(position.start.line, column, column + 1))
                    .with_fix(Fix::new(
                        Range::single_line(position.start.line, column, column + 1),
                        "",
                    )),
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
        NoTrailingPunctuationHeading.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_clean_heading() {
        assert!(run("# Title\n").is_empty());
    }

    #[test]
    fn question_marks_are_allowed_by_default() {
        assert!(run("# What now?\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_trailing_period() {
        let diagnostics = run("# Title.\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "");
    }
}
