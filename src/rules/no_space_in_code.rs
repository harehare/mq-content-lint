//! MD038: spaces just inside code span backticks (`` ` text ` `` instead of `` `text` ``).
//! CommonMark itself strips exactly one leading/trailing space if the span both starts and ends
//! with one, so this inspects the raw source (not [`mq_markdown::CodeInline::value`], which has
//! that normalization already applied) to catch any left over.

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
            let Some(raw) = crate::fix::slice(source, &byte_index, position.clone().into()) else {
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
                    position.start.column + backtick_len,
                    position.end.column - backtick_len,
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
}
