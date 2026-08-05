//! MD019: multiple spaces between the `#`s and the text of an ATX heading.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMultipleSpaceAtx;

impl Rule for NoMultipleSpaceAtx {
    fn id(&self) -> RuleId {
        RuleId::NoMultipleSpaceAtx
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let Some((_, line)) = crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line) else {
                return;
            };
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if !trimmed.starts_with('#') {
                return;
            }
            let hash_len = trimmed.chars().take_while(|&c| c == '#').count();
            let after = &trimmed[hash_len..];
            let space_len = after.chars().take_while(|&c| c == ' ').count();
            if space_len > 1 {
                let start_col = indent + hash_len + 1;
                let end_col = start_col + space_len;
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoMultipleSpaceAtx, self.default_severity())
                        .with_range(Range::single_line(position.start.line, start_col, end_col))
                        .with_fix(Fix::new(
                            Range::single_line(position.start.line, start_col, end_col),
                            " ",
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
        NoMultipleSpaceAtx.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_single_space() {
        assert!(run("# Title\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_multiple_spaces() {
        let diagnostics = run("#   Title\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }
}
