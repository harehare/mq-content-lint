//! MD018: a line that looks like an ATX heading attempt (`#Title`, no space after the `#`s) but
//! is missing the required space, so CommonMark parses it as an ordinary paragraph instead of a
//! heading — meaning it never shows up as a [`mq_markdown::Node::Heading`] at all. This rule
//! scans raw lines (skipping fenced code blocks) rather than the AST for exactly that reason.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoMissingSpaceAtx;

/// If `line` looks like a `#`-heading missing its space, returns the hash run length.
fn missing_space_hash_len(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let hash_len = trimmed.chars().take_while(|&c| c == '#').count();
    if hash_len == 0 || hash_len > 6 {
        return None;
    }
    let rest = &trimmed[hash_len..];
    let next = rest.chars().next()?;
    (next != ' ' && next != '\t' && next != '#').then_some(hash_len)
}

impl Rule for NoMissingSpaceAtx {
    fn id(&self) -> RuleId {
        RuleId::NoMissingSpaceAtx
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut code_ranges = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Code(code) = node
                && let Some(position) = &code.position
            {
                code_ranges.push((position.start.line, position.end.line));
            }
        });

        crate::text::numbered_lines(source)
            .filter_map(|(line_number, line)| {
                if code_ranges
                    .iter()
                    .any(|(start, end)| *start <= line_number && line_number <= *end)
                {
                    return None;
                }
                let hash_len = missing_space_hash_len(line)?;
                let indent = line.len() - line.trim_start().len();
                let column = indent + hash_len + 1;
                Some(
                    Diagnostic::new(LintMessage::NoMissingSpaceAtx, self.default_severity())
                        .with_range(Range::at(line_number, column))
                        .with_fix(Fix::new(Range::at(line_number, column), " ")),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        NoMissingSpaceAtx.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_proper_heading() {
        assert!(run("# Title\n").is_empty());
    }

    #[test]
    fn flags_a_heading_missing_its_space() {
        let diagnostics = run("#Title\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, " ");
    }

    #[test]
    fn ignores_hashtag_like_text_inside_fenced_code() {
        assert!(run("```\n#not-a-heading\n```\n").is_empty());
    }

    #[test]
    fn ignores_lines_with_more_than_six_hashes() {
        assert!(run("#######Text\n").is_empty());
    }
}
