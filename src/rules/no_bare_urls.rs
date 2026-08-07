//! MD034: a bare URL (`https://example.com` with no `<...>` or `[text](...)` wrapping) in body
//! text.
//!
//! Scans raw lines (skipping fenced code) rather than the AST: `mq-markdown`'s GFM autolink
//! extension turns *every* bare URL into a [`mq_markdown::Node::Link`] regardless of whether the
//! source wrapped it in `<>` — by the time it's a `Link` node there's no way to tell whether the
//! source already used proper syntax, so this instead looks at what immediately surrounds each
//! `http(s)://` occurrence in the raw text: already preceded by `<` (autolink) or `(` (a link's
//! destination) is left alone, everything else is a bare URL.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoBareUrls;

const HTTP_PREFIX: &[char] = &['h', 't', 't', 'p', ':', '/', '/'];
const HTTPS_PREFIX: &[char] = &['h', 't', 't', 'p', 's', ':', '/', '/'];

fn find_urls(line: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        // Slice-based prefix checks instead of collecting `chars[i..]` into a new `String` on
        // every position — the latter is O(remaining line length) per position, making a single
        // line's scan O(length²) overall.
        if !chars[i..].starts_with(HTTP_PREFIX) && !chars[i..].starts_with(HTTPS_PREFIX) {
            i += 1;
            continue;
        }
        let preceding_char = if i == 0 { None } else { Some(chars[i - 1]) };
        let mut j = i;
        while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '>' && chars[j] != ')' {
            j += 1;
        }
        if !matches!(preceding_char, Some('<') | Some('(')) {
            result.push((i, j));
        }
        i = j;
    }
    result
}

impl Rule for NoBareUrls {
    fn id(&self) -> RuleId {
        RuleId::NoBareUrls
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
        let code_lines = crate::text::CodeBlockLines::new(code_ranges);

        let mut diagnostics = Vec::new();
        for (line_number, line) in crate::text::numbered_lines(source) {
            if code_lines.contains(line_number) {
                continue;
            }
            for (start, end) in find_urls(line) {
                let url: String = line.chars().skip(start).take(end - start).collect();
                let range = Range::single_line(line_number, start + 1, end + 1);
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoBareUrls { url: url.clone() }, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, format!("<{url}>"))),
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
        NoBareUrls.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_a_proper_link() {
        assert!(run("See [the site](https://example.com) for more.\n").is_empty());
    }

    #[test]
    fn no_diagnostics_for_an_autolink() {
        assert!(run("See <https://example.com> for more.\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_bare_url() {
        let diagnostics = run("See https://example.com for more.\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].fix.as_ref().unwrap().replacement,
            "<https://example.com>"
        );
    }

    #[test]
    fn ignores_fenced_code_blocks() {
        assert!(run("```\nhttps://example.com\n```\n").is_empty());
    }
}
