//! MD009: trailing whitespace at the end of a line. Exactly two trailing spaces are allowed by
//! default (a Markdown hard line break); configure the allowed count via
//! `[rules.no_trailing_spaces] br_spaces`, or set it to `0` to disallow the hard-break form too.

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoTrailingSpaces;

const DEFAULT_BR_SPACES: usize = 2;

impl Rule for NoTrailingSpaces {
    fn id(&self) -> RuleId {
        RuleId::NoTrailingSpaces
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let br_spaces = config
            .rule_options(self.id())
            .get_usize("br_spaces")
            .unwrap_or(DEFAULT_BR_SPACES);

        crate::text::numbered_lines(source)
            .filter_map(|(line_number, line)| {
                let trimmed = line.trim_end_matches([' ', '\t']);
                let trailing_len = line.chars().count() - trimmed.chars().count();
                if trailing_len == 0 || trailing_len == br_spaces {
                    return None;
                }
                let start_col = trimmed.chars().count() + 1;
                let range = Range::single_line(line_number, start_col, start_col + trailing_len);
                Some(
                    Diagnostic::new(LintMessage::NoTrailingSpaces, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, "")),
                )
            })
            .collect()
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["br_spaces"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        NoTrailingSpaces.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_clean_lines() {
        assert!(run("Hello\nWorld\n").is_empty());
    }

    #[test]
    fn allows_exactly_two_trailing_spaces() {
        assert!(run("Hello  \nWorld\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_single_trailing_space() {
        let diagnostics = run("Hello \nWorld\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "");
    }

    #[test]
    fn flags_more_than_two_trailing_spaces() {
        assert_eq!(run("Hello   \n").len(), 1);
    }
}
