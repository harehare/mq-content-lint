//! MD012: more than one consecutive blank line. Configurable maximum via
//! `[rules.no_multiple_blanks] maximum` (default 1).

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMultipleBlanks;

const DEFAULT_MAXIMUM: usize = 1;

impl Rule for NoMultipleBlanks {
    fn id(&self) -> RuleId {
        RuleId::NoMultipleBlanks
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let maximum = config
            .rule_options(self.id())
            .get_usize("maximum")
            .unwrap_or(DEFAULT_MAXIMUM);
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            if !lines[i].trim().is_empty() {
                i += 1;
                continue;
            }
            let start = i;
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            let run_len = i - start;
            if run_len > maximum {
                let extra_start_line = start + maximum + 1;
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::NoMultipleBlanks { count: run_len },
                        self.default_severity(),
                    )
                    .with_range(Range::single_line(start + 1, 1, 1))
                    .with_fix(Fix::new(
                        Range {
                            start_line: extra_start_line,
                            start_column: 1,
                            end_line: i + 1,
                            end_column: 1,
                        },
                        "",
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
        NoMultipleBlanks.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_single_blank_lines() {
        assert!(run("One\n\nTwo\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_multiple_blank_lines() {
        let diagnostics = run("One\n\n\n\nTwo\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, LintMessage::NoMultipleBlanks { count: 3 });
        let fixed = crate::fix::apply_fixes("One\n\n\n\nTwo\n", &[diagnostics[0].fix.clone().unwrap()]);
        assert_eq!(fixed, "One\n\nTwo\n");
    }
}
