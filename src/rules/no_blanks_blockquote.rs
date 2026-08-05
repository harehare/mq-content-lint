//! MD028: a blank (or bare `>`) line between two blockquote lines. CommonMark treats this as a
//! lazy continuation of a single blockquote with an awkward gap rather than two separate quotes,
//! which is rarely what's intended. Not auto-fixable — whether to merge or split the quote is an
//! editorial call.

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoBlanksBlockquote;

fn is_blockquote_line(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn is_blank_or_bare_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed == ">"
}

impl Rule for NoBlanksBlockquote {
    fn id(&self) -> RuleId {
        RuleId::NoBlanksBlockquote
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        for i in 0..lines.len() {
            if !is_blank_or_bare_marker(lines[i]) {
                continue;
            }
            let before = i > 0 && is_blockquote_line(lines[i - 1]);
            let after = i + 1 < lines.len() && is_blockquote_line(lines[i + 1]);
            if before && after {
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoBlanksBlockquote, self.default_severity())
                        .with_range(Range::single_line(i + 1, 1, lines[i].chars().count() + 1)),
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
        NoBlanksBlockquote.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_clean_blockquote() {
        assert!(run("> line one\n> line two\n").is_empty());
    }

    #[test]
    fn flags_a_blank_line_between_quote_lines() {
        let diagnostics = run("> line one\n\n> line two\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn no_diagnostics_when_blockquote_simply_ends() {
        assert!(run("> quoted\n\nParagraph after.\n").is_empty());
    }
}
