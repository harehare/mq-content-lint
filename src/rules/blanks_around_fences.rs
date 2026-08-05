//! MD031: fenced code blocks should be surrounded by a blank line on each side.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct BlanksAroundFences;

impl Rule for BlanksAroundFences {
    fn id(&self) -> RuleId {
        RuleId::BlanksAroundFences
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let lines: Vec<&str> = source.lines().collect();
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Code(code) = node else { return };
            if !code.fence {
                return;
            }
            let Some(position) = &code.position else { return };
            let start = position.start.line;
            let end = position.end.line;

            if start > 1 && lines.get(start - 2).is_some_and(|l| !l.trim().is_empty()) {
                diagnostics.push(
                    Diagnostic::new(LintMessage::BlanksAroundFences { above: true }, self.default_severity())
                        .with_range(Range::at(start, 1))
                        .with_fix(Fix::new(Range::at(start, 1), "\n")),
                );
            }
            if end < lines.len() && lines.get(end).is_some_and(|l| !l.trim().is_empty()) {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::BlanksAroundFences { above: false },
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
        BlanksAroundFences.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_when_properly_surrounded() {
        assert!(run("Intro\n\n```\ncode\n```\n\nBody\n").is_empty());
    }

    #[test]
    fn flags_missing_blank_lines() {
        let diagnostics = run("Intro\n```\ncode\n```\nBody\n");
        assert_eq!(diagnostics.len(), 2);
    }
}
