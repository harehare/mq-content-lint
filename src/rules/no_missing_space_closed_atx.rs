//! MD020: a closed ATX heading (`# Title#`) missing the space before its closing `#`s.
//!
//! Heuristic, like markdownlint's own MD020: a heading whose raw line ends with a `#`-run not
//! preceded by whitespace is treated as an attempted closed-ATX heading missing its delimiter.
//! A heading that legitimately ends its text in `#` (e.g. `# Learn C#`) will false-positive here;
//! disable the rule per-document if that's a recurring problem.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMissingSpaceClosedAtx;

impl Rule for NoMissingSpaceClosedAtx {
    fn id(&self) -> RuleId {
        RuleId::NoMissingSpaceClosedAtx
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
            let trimmed_end = line.trim_end();
            let core = trimmed_end.trim_end_matches('#');
            let closing_len = trimmed_end.len() - core.len();
            if closing_len == 0 || core.is_empty() {
                return;
            }
            if !core.ends_with(' ') && !core.ends_with('\t') {
                let column = core.chars().count() + 1;
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoMissingSpaceClosedAtx, self.default_severity())
                        .with_range(Range::at(position.start.line, column))
                        .with_fix(Fix::new(Range::at(position.start.line, column), " ")),
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
        NoMissingSpaceClosedAtx.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_properly_spaced_closed_atx() {
        assert!(run("# Title #\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_missing_space_before_closing_hash() {
        let diagnostics = run("# Title#\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }

    #[test]
    fn ignores_plain_atx_headings() {
        assert!(run("# Title\n").is_empty());
    }
}
