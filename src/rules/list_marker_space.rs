//! MD030: the number of spaces after a list marker (`-`, `*`, `+`, or `1.`) should be consistent
//! — 1 by default, configurable via `[rules.list_marker_space] spaces`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct ListMarkerSpace;

const DEFAULT_SPACES: usize = 1;

/// Returns the length in characters of the list marker itself (bullet, or `<digits><delim>`).
fn marker_len(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let first = trimmed.chars().next()?;
    if "-*+".contains(first) {
        return Some(1);
    }
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    matches!(trimmed.chars().nth(digits), Some('.') | Some(')')).then_some(digits + 1)
}

impl Rule for ListMarkerSpace {
    fn id(&self) -> RuleId {
        RuleId::ListMarkerSpace
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let expected = config
            .rule_options(self.id())
            .get_usize("spaces")
            .unwrap_or(DEFAULT_SPACES);
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        for node in &doc.nodes {
            let Node::List(list) = node else { continue };
            let Some(position) = &list.position else { continue };
            let Some(line) = lines.get(position.start.line) else {
                continue;
            };
            let indent = line.len() - line.trim_start().len();
            let Some(marker) = marker_len(line) else { continue };
            let after = &line[indent + marker..];
            let space_len = after.chars().take_while(|c| *c == ' ').count();

            if space_len != expected && !after.trim().is_empty() {
                let start_col = indent + marker + 1;
                let range = Range::single_line(position.start.line, start_col, start_col + space_len);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::ListMarkerSpace {
                            expected,
                            found: space_len,
                        },
                        self.default_severity(),
                    )
                    .with_range(range)
                    .with_fix(Fix::new(range, " ".repeat(expected))),
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
        ListMarkerSpace.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_single_space() {
        assert!(run("- one\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_multiple_spaces_after_marker() {
        let diagnostics = run("-   one\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }
}
