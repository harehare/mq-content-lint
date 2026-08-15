//! MD036: a line that is *entirely* emphasized or strong text (e.g. a lone `**Section**` between
//! blank lines) looks like it was meant to be a heading. Not auto-fixable — promoting it to a
//! heading requires picking a level, which is an editorial call.
//!
//! `mq_markdown::Position::column` counts UTF-8 bytes, not `char`s — see
//! [`super::no_space_in_code`]'s doc comment — so `position.start.column`/`.end.column` are
//! converted through [`crate::text::LineByteIndex::char_column`] before indexing into the line's
//! `char`s, or the two would silently misalign on a line containing multi-byte characters.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct NoEmphasisAsHeading;

impl Rule for NoEmphasisAsHeading {
    fn id(&self) -> RuleId {
        RuleId::NoEmphasisAsHeading
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
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
        let lines: Vec<&str> = source.lines().collect();
        let byte_index = crate::text::LineByteIndex::new(source);
        let mut diagnostics = Vec::new();

        for node in &doc.nodes {
            if !matches!(node, Node::Strong(_) | Node::Emphasis(_)) {
                continue;
            }
            let Some(position) = node.position() else { continue };
            if position.start.line != position.end.line {
                continue;
            }
            let Some(line) = lines.get(position.start.line - 1) else {
                continue;
            };
            let Some(start_column) = byte_index.char_column(position.start.line, position.start.column) else {
                continue;
            };
            let Some(end_column) = byte_index.char_column(position.end.line, position.end.column) else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            if start_column == 0 || end_column - 1 > chars.len() {
                continue;
            }
            let span: String = chars[(start_column - 1)..(end_column - 1)].iter().collect();
            let trimmed_line = line.trim();

            if !trimmed_line.is_empty() && span.trim() == trimmed_line {
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::NoEmphasisAsHeading { text: node.value() },
                        self.default_severity(),
                    )
                    .with_range(position),
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
        NoEmphasisAsHeading.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_emphasis_within_a_sentence() {
        assert!(run("This is **important** context.\n").is_empty());
    }

    #[test]
    fn flags_a_standalone_bold_line() {
        let diagnostics = run("Intro\n\n**Section Title**\n\nBody\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::NoEmphasisAsHeading {
                text: "Section Title".to_string()
            }
        );
    }

    #[test]
    fn no_diagnostics_for_a_real_heading() {
        assert!(run("# Section Title\n").is_empty());
    }

    #[test]
    fn flags_a_standalone_bold_line_of_multi_byte_text() {
        let diagnostics = run("Intro\n\n**従うように**\n\nBody\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::NoEmphasisAsHeading {
                text: "従うように".to_string()
            }
        );
    }
}
