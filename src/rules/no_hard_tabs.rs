//! MD010: hard tab characters, which render inconsistently across editors/viewers. Fixed by
//! replacing each tab with `[rules.no_hard_tabs] spaces` spaces (default 4).

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoHardTabs;

const DEFAULT_SPACES: usize = 4;

impl Rule for NoHardTabs {
    fn id(&self) -> RuleId {
        RuleId::NoHardTabs
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        _doc: &mq_markdown::Markdown,
        source: &str,
        config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let spaces = config
            .rule_options(self.id())
            .get_usize("spaces")
            .unwrap_or(DEFAULT_SPACES);
        let mut diagnostics = Vec::new();

        for (line_number, line) in crate::text::numbered_lines(source) {
            for (column, ch) in line.chars().enumerate() {
                if ch == '\t' {
                    let range = Range::single_line(line_number, column + 1, column + 2);
                    diagnostics.push(
                        Diagnostic::new(LintMessage::NoHardTabs, self.default_severity())
                            .with_range(range)
                            .with_fix(Fix::new(range, " ".repeat(spaces))),
                    );
                }
            }
        }
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["spaces"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        NoHardTabs.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_without_tabs() {
        assert!(run("Hello World\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_tab() {
        let diagnostics = run("Hello\tWorld\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "    ");
    }
}
