//! MD037: spaces just inside emphasis markers (`* text *` instead of `*text*`). CommonMark's
//! flanking rules mean a `*`/`_` immediately followed by whitespace can't open emphasis at all,
//! so `* text *` never becomes an [`mq_markdown::Node::Emphasis`] — this rule scans raw lines
//! (skipping fenced code) for the pattern instead, the same reason [`super::no_missing_space_atx`]
//! does. Heuristic, like markdownlint's own MD037: a marker used for something else entirely
//! (e.g. multiplication, `5 * 3 * 2`) can false-positive if it happens to bracket a word pair.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct NoSpaceInEmphasis;

/// Finds `<marker> <content> <marker>` spans (space just inside a `*`/`_` pair) in `line`,
/// returning `(char_start, char_end, marker, inner_trimmed)`. `marker` comes straight from the
/// scan rather than making a caller re-derive it with a second, position-dependent lookup.
fn find_spaced_emphasis(line: &str) -> Vec<(usize, usize, char, String)> {
    let chars: Vec<char> = line.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let marker = chars[i];
        if (marker != '*' && marker != '_') || i + 2 >= chars.len() || chars[i + 1] != ' ' {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < chars.len() && chars[j] != marker {
            j += 1;
        }
        if j >= chars.len() || j < 2 || chars[j - 1] != ' ' {
            i += 1;
            continue;
        }
        let inner: String = chars[i + 1..j].iter().collect();
        if inner.trim().chars().any(char::is_alphanumeric) {
            result.push((i, j + 1, marker, inner.trim().to_string()));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    result
}

impl Rule for NoSpaceInEmphasis {
    fn id(&self) -> RuleId {
        RuleId::NoSpaceInEmphasis
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
            for (start, end, marker, inner) in find_spaced_emphasis(line) {
                let range = Range::single_line(line_number, start + 1, end + 1);
                diagnostics.push(
                    Diagnostic::new(LintMessage::NoSpaceInEmphasis, self.default_severity())
                        .with_range(range)
                        .with_fix(Fix::new(range, format!("{marker}{inner}{marker}"))),
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
        NoSpaceInEmphasis.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_clean_emphasis() {
        assert!(run("*text*\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_spaces_inside_markers() {
        let diagnostics = run("Some * text * here.\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "*text*");
    }

    #[test]
    fn ignores_fenced_code_blocks() {
        assert!(run("```\n* text *\n```\n").is_empty());
    }
}
