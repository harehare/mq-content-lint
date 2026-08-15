//! MD047: a file should end with exactly one newline character — not zero (no trailing newline
//! at all) and not several (trailing blank lines).

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct SingleTrailingNewline;

impl Rule for SingleTrailingNewline {
    fn id(&self) -> RuleId {
        RuleId::SingleTrailingNewline
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        _doc: &mq_markdown::Markdown,
        source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        if source.is_empty() {
            return Vec::new();
        }

        let trimmed = source.trim_end_matches('\n');
        let trailing_newlines = source.len() - trimmed.len();
        if trailing_newlines == 1 {
            return Vec::new();
        }

        let lines_before = trimmed.matches('\n').count() + 1;
        let last_line_len = trimmed.rsplit('\n').next().unwrap_or("").chars().count();
        let end_of_content = Range::at(lines_before, last_line_len + 1);

        vec![
            Diagnostic::new(LintMessage::SingleTrailingNewline, self.default_severity())
                .with_range(end_of_content)
                .with_fix(Fix::new(
                    Range {
                        start_line: end_of_content.start_line,
                        start_column: end_of_content.start_column,
                        end_line: end_of_content.start_line + trailing_newlines.max(1),
                        end_column: 1,
                    },
                    "\n",
                )),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        SingleTrailingNewline.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_exactly_one_trailing_newline() {
        assert!(run("Hello\n").is_empty());
    }

    #[test]
    fn flags_a_missing_trailing_newline() {
        let diagnostics = run("Hello");
        assert_eq!(diagnostics.len(), 1);
        let fixed = crate::fix::apply_fixes("Hello", &[diagnostics[0].fix.clone().unwrap()]);
        assert_eq!(fixed, "Hello\n");
    }

    #[test]
    fn flags_and_fixes_multiple_trailing_newlines() {
        let diagnostics = run("Hello\n\n\n");
        assert_eq!(diagnostics.len(), 1);
        let fixed = crate::fix::apply_fixes("Hello\n\n\n", &[diagnostics[0].fix.clone().unwrap()]);
        assert_eq!(fixed, "Hello\n");
    }
}
