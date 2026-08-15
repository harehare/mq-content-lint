//! MD023: a heading must start at the beginning of its line (no leading whitespace).
//!
//! `mq-markdown`'s heading position always reports column 1 regardless of the (CommonMark-legal,
//! up to 3 spaces) leading indentation in the source, so this inspects the raw line rather than
//! `Position::start.column`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct HeadingStartLeft;

impl Rule for HeadingStartLeft {
    fn id(&self) -> RuleId {
        RuleId::HeadingStartLeft
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let Some(line) = lines.get(position.start.line) else {
                return;
            };
            let indent = line.len() - line.trim_start().len();
            if indent > 0 {
                let range = Range::single_line(position.start.line, 1, indent + 1);
                diagnostics.push(
                    Diagnostic::new(LintMessage::HeadingStartLeft, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, "")),
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
        HeadingStartLeft.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_left_aligned_heading() {
        assert!(run("# Title\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_indented_heading() {
        let diagnostics = run("   # Title\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "");
    }
}
