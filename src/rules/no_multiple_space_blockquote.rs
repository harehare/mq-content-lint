//! MD027: multiple spaces after the blockquote `>` marker.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMultipleSpaceBlockquote;

impl Rule for NoMultipleSpaceBlockquote {
    fn id(&self) -> RuleId {
        RuleId::NoMultipleSpaceBlockquote
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut ranges = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Blockquote(bq) = node
                && let Some(position) = &bq.position
            {
                ranges.push((position.start.line, position.end.line));
            }
        });

        let mut diagnostics = Vec::new();
        for (line_number, line) in crate::text::numbered_lines(source) {
            if !ranges
                .iter()
                .any(|(start, end)| *start <= line_number && line_number <= *end)
            {
                continue;
            }
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if !trimmed.starts_with('>') {
                continue;
            }
            let after = &trimmed[1..];
            let space_len = after.chars().take_while(|c| *c == ' ').count();
            if space_len > 1 {
                let start_col = indent + 2;
                let range = Range::single_line(line_number, start_col, start_col + space_len);
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoMultipleSpaceBlockquote, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, " ")),
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
        NoMultipleSpaceBlockquote.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_single_space() {
        assert!(run("> quote\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_multiple_spaces() {
        let diagnostics = run(">   quote\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }
}
