//! MD021: multiple spaces before the closing `#`s of a closed ATX heading (`# Title   #`).

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMultipleSpaceClosedAtx;

impl Rule for NoMultipleSpaceClosedAtx {
    fn id(&self) -> RuleId {
        RuleId::NoMultipleSpaceClosedAtx
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let Some(line) = lines.get(position.start.line) else {
                return;
            };
            let trimmed_end = line.trim_end();
            let core = trimmed_end.trim_end_matches('#');
            let closing_len = trimmed_end.len() - core.len();
            if closing_len == 0 {
                return;
            }
            let core_trimmed = core.trim_end_matches(' ');
            let space_len = core.len() - core_trimmed.len();
            if space_len > 1 {
                let start_col = core_trimmed.chars().count() + 1;
                let end_col = start_col + space_len;
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoMultipleSpaceClosedAtx, self.default_severity())
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
        NoMultipleSpaceClosedAtx.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_single_space_before_closing_hash() {
        assert!(run("# Title #\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_multiple_spaces_before_closing_hash() {
        let diagnostics = run("# Title   #\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }
}
