//! MD052: a reference-style link or image (`[text][label]` or the collapsed `[text][]`) whose
//! label has no matching `[label]: url` definition anywhere in the document.
//!
//! An *undefined* reference never becomes a [`mq_markdown::Node::LinkRef`]/`ImageRef` at all —
//! CommonMark falls back to treating it as literal text — so this scans
//! [`mq_markdown::Node::Text`] content for the `[text][label]` syntax directly; anything that
//! *did* resolve is already a `LinkRef`/`ImageRef` node, not `Text`, so it's naturally excluded.
//! The bare shortcut form `[text]` (no second bracket) is intentionally not checked here: it's
//! indistinguishable from ordinary bracketed prose without much higher false-positive risk.
//!
//! `mq_markdown::Position::column` counts UTF-8 bytes, not `char`s — see
//! [`super::no_space_in_code`]'s doc comment — so `position.start.column` is converted through
//! [`crate::text::LineByteIndex::char_column`] before it's combined with
//! `find_reference_patterns`'s char-counted offsets, or the two would silently misalign on text
//! containing multi-byte characters.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct ReferenceLinksImages;

/// Finds `[text][label]` / `[text][]` spans in `text`, returning `(char_start, char_end, label)`
/// (an empty second bracket collapses to the first bracket's content, per CommonMark).
fn find_reference_patterns(text: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '[' {
            i += 1;
            continue;
        }
        let Some(close1) = chars[i + 1..]
            .iter()
            .position(|&c| c == ']' || c == '[')
            .map(|p| i + 1 + p)
        else {
            i += 1;
            continue;
        };
        if chars[close1] != ']' || close1 + 1 >= chars.len() || chars[close1 + 1] != '[' {
            i += 1;
            continue;
        }
        let Some(close2) = chars[close1 + 2..]
            .iter()
            .position(|&c| c == ']')
            .map(|p| close1 + 2 + p)
        else {
            i += 1;
            continue;
        };
        let first: String = chars[i + 1..close1].iter().collect();
        let second: String = chars[close1 + 2..close2].iter().collect();
        if first.is_empty() {
            i += 1;
            continue;
        }
        let label = if second.is_empty() { first } else { second };
        result.push((i, close2 + 1, label));
        i = close2 + 1;
    }
    result
}

impl Rule for ReferenceLinksImages {
    fn id(&self) -> RuleId {
        RuleId::ReferenceLinksImages
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let byte_index = crate::text::LineByteIndex::new(source);
        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Text(text) = node else { return };
            let Some(position) = &text.position else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(start_column) = byte_index.char_column(position.start.line, position.start.column) else {
                return;
            };
            for (start, end, label) in find_reference_patterns(&text.value) {
                let range = Range::single_line(position.start.line, start_column + start, start_column + end);
                diagnostics.push(
                    Diagnostic::new(LintMessage::ReferenceLinksImages { label }, self.default_severity())
                        .with_range(range),
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
        ReferenceLinksImages.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_definition_exists() {
        assert!(run("[text][ref]\n\n[ref]: https://example.com\n").is_empty());
    }

    #[test]
    fn flags_an_undefined_reference() {
        let diagnostics = run("[text][missing]\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::ReferenceLinksImages {
                label: "missing".to_string()
            }
        );
    }

    #[test]
    fn flags_an_undefined_collapsed_reference() {
        let diagnostics = run("[missing][]\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_an_undefined_reference_after_multi_byte_text() {
        let diagnostics = run("従うように、[text][missing]\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::ReferenceLinksImages {
                label: "missing".to_string()
            }
        );
    }
}
