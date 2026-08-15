//! MD022: headings should be surrounded by a blank line on each side (except at the very start
//! or end of the file, where there's nothing to separate it from).

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct BlanksAroundHeadings;

impl Rule for BlanksAroundHeadings {
    fn id(&self) -> RuleId {
        RuleId::BlanksAroundHeadings
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
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let start = position.start.line;
            let end = position.end.line;

            if start > 1 && lines.get(start - 2).is_some_and(|l| !l.trim().is_empty()) {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::BlanksAroundHeadings { above: true },
                        self.default_severity(),
                    )
                    .with_range(Range::at(start, 1))
                    .with_fix(Fix::new(Range::at(start, 1), "\n")),
                );
            }
            if end < lines.len() && lines.get(end).is_some_and(|l| !l.trim().is_empty()) {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::BlanksAroundHeadings { above: false },
                        self.default_severity(),
                    )
                    .with_range(Range::at(end, lines[end - 1].chars().count() + 1))
                    .with_fix(Fix::new(Range::at(end + 1, 1), "\n")),
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
        BlanksAroundHeadings.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_properly_surrounded() {
        assert!(run("Intro\n\n# Title\n\nBody\n").is_empty());
    }

    #[test]
    fn no_diagnostics_at_start_and_end_of_file() {
        assert!(run("# Title\n").is_empty());
    }

    #[test]
    fn flags_missing_blank_line_above_and_below() {
        let diagnostics = run("Intro\n# Title\nBody\n");
        assert_eq!(diagnostics.len(), 2);
        assert!(matches!(
            diagnostics[0].message,
            LintMessage::BlanksAroundHeadings { above: true }
        ));
        assert!(matches!(
            diagnostics[1].message,
            LintMessage::BlanksAroundHeadings { above: false }
        ));
    }
}
