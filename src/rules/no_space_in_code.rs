//! MD038: spaces just inside code span backticks (`` ` text ` `` instead of `` `text` ``).
//! CommonMark itself strips exactly one leading/trailing space if the span both starts and ends
//! with one, so this inspects the raw source (not [`mq_markdown::CodeInline::value`], which has
//! that normalization already applied) to catch any left over.
//!
//! `mq_markdown::Position::column` counts UTF-8 bytes, not `char`s (confirmed empirically — e.g.
//! a code span containing "従うように" reports an `end.column` matching the line's byte length,
//! not its character count). Every column here is converted through
//! [`crate::text::LineByteIndex::char_column`] before use, so it lines up with `Range`/
//! `crate::fix::slice`'s char-counted convention; skipping that conversion is what used to panic
//! ("byte index is not a char boundary") on a code span containing multi-byte characters.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoSpaceInCode;

impl Rule for NoSpaceInCode {
    fn id(&self) -> RuleId {
        RuleId::NoSpaceInCode
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let byte_index = crate::text::LineByteIndex::new(source);
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::CodeInline(_) = node else { return };
            let Some(position) = node.position() else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(start_column) = byte_index.char_column(position.start.line, position.start.column) else {
                return;
            };
            let Some(end_column) = byte_index.char_column(position.end.line, position.end.column) else {
                return;
            };
            let span = Range::single_line(position.start.line, start_column, end_column);
            let Some(raw) = crate::fix::slice(source, &byte_index, span) else {
                return;
            };
            let backtick_len = raw.chars().take_while(|&c| c == '`').count();
            if backtick_len == 0 || raw.len() < backtick_len * 2 {
                return;
            }
            let inner = &raw[backtick_len..raw.len() - backtick_len];
            let trimmed = inner.trim_matches(' ');
            if inner != trimmed && !trimmed.is_empty() {
                let range = Range::single_line(
                    position.start.line,
                    start_column + backtick_len,
                    end_column - backtick_len,
                );
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoSpaceInCode, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, trimmed.to_string())),
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
        NoSpaceInCode.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_clean_code_span() {
        assert!(run("`text`\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_extra_spaces() {
        let diagnostics = run("`  text  `\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "text");
    }

    #[test]
    fn no_diagnostics_for_a_clean_multi_byte_code_span() {
        assert!(run("`従うように`\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_extra_spaces_around_multi_byte_content() {
        let diagnostics = run("` 従うように `\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "従うように");
    }
}
