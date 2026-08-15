//! MD011: reversed link syntax, `(text)[url]` instead of `[text](url)` — a typo that CommonMark
//! leaves as plain text rather than erroring on, so it's easy to miss without a linter.
//!
//! Scans raw lines (skipping fenced code) rather than [`mq_markdown::Node::Text`] content: GFM's
//! autolink extension can split the `url` portion of `(text)[url]` off into its own `Link` node
//! (when `url` looks enough like a bare URL), which would otherwise hide the pattern from a
//! Text-node-only scan.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoReversedLinks;

/// Finds `(text)[url]` spans in `line`, returning `(char_start, char_end, text, url)`.
fn find_reversed_links(line: &str) -> Vec<(usize, usize, String, String)> {
    let chars: Vec<char> = line.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '(' {
            i += 1;
            continue;
        }
        let Some(close_paren) = chars[i + 1..]
            .iter()
            .position(|&c| c == ')' || c == '(')
            .map(|p| i + 1 + p)
        else {
            i += 1;
            continue;
        };
        if chars[close_paren] != ')' || close_paren + 1 >= chars.len() || chars[close_paren + 1] != '[' {
            i += 1;
            continue;
        }
        let Some(close_bracket) = chars[close_paren + 2..]
            .iter()
            .position(|&c| c == ']')
            .map(|p| close_paren + 2 + p)
        else {
            i += 1;
            continue;
        };
        let link_text: String = chars[i + 1..close_paren].iter().collect();
        let url: String = chars[close_paren + 2..close_bracket].iter().collect();
        if !link_text.is_empty() && !url.is_empty() && !url.contains(' ') {
            result.push((i, close_bracket + 1, link_text, url));
            i = close_bracket + 1;
        } else {
            i += 1;
        }
    }
    result
}

impl Rule for NoReversedLinks {
    fn id(&self) -> RuleId {
        RuleId::NoReversedLinks
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
        let mut code_ranges = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Code(code) = node
                && let Some(position) = &code.position
            {
                code_ranges.push((position.start.line, position.end.line));
            }
        });
        let code_lines = crate::text::CodeBlockLines::new(code_ranges);

        let mut diagnostics = Vec::new();
        for (line_number, line) in crate::text::numbered_lines(source) {
            if code_lines.contains(line_number) {
                continue;
            }
            for (start, end, link_text, url) in find_reversed_links(line) {
                let range = Range::single_line(line_number, start + 1, end + 1);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::NoReversedLinks {
                            text: format!("({link_text})[{url}]"),
                        },
                        self.default_severity(),
                    )
                    .with_range(range)
                    .with_fix(Fix::new(range, format!("[{link_text}]({url})"))),
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
        NoReversedLinks.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_a_correct_link() {
        assert!(run("See [the site](https://example.com).\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_reversed_link() {
        let diagnostics = run("See (the site)[https://example.com].\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].fix.as_ref().unwrap().replacement,
            "[the site](https://example.com)"
        );
    }

    #[test]
    fn ignores_plain_parenthetical_text() {
        assert!(run("This (is just text) with brackets [elsewhere].\n").is_empty());
    }

    #[test]
    fn ignores_fenced_code_blocks() {
        assert!(run("```\n(text)[url]\n```\n").is_empty());
    }
}
