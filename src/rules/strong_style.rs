//! MD050: strong marker (`**text**` vs. `__text__`) should be consistent across the document.
//! `[rules.strong_style] style` accepts `"consistent"` (default), `"asterisk"`, or
//! `"underscore"`.
//!
//! `mq_markdown::Position::column` counts UTF-8 bytes, not `char`s — see
//! [`super::no_space_in_code`]'s doc comment — so `position` is converted to a char-counted
//! `Range` through [`crate::text::LineByteIndex::char_column`] before it's used for
//! [`crate::fix::slice`] or as the diagnostic's own `Range`/`Fix`, or strong text wrapping
//! multi-byte content would misalign both.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct StrongStyle;

fn configured_marker(style: &str) -> Option<&'static str> {
    match style {
        "asterisk" => Some("**"),
        "underscore" => Some("__"),
        _ => None,
    }
}

impl Rule for StrongStyle {
    fn id(&self) -> RuleId {
        RuleId::StrongStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let mut expected = config
            .rule_options(self.id())
            .get_str("style")
            .and_then(configured_marker);
        let mut diagnostics = Vec::new();

        let byte_index = crate::text::LineByteIndex::new(source);
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Strong(_) = node else { return };
            let Some(position) = node.position() else { return };
            if position.start.line != position.end.line {
                return;
            }
            let Some(start_column) = byte_index.char_column(position.start.line, position.start.column) else {
                return;
            };
            let Some(end_column) = byte_index.char_column(position.end.line, position.end.column) else {
                return;
            };
            let range = Range::single_line(position.start.line, start_column, end_column);
            let Some(raw) = crate::fix::slice(source, &byte_index, range) else {
                return;
            };
            if raw.len() < 4 {
                return;
            }
            let found = &raw[..2];
            if (found != "**" && found != "__") || &raw[raw.len() - 2..] != found {
                return;
            }
            let expected_marker = *expected.get_or_insert(found);

            if found != expected_marker {
                let inner = &raw[2..raw.len() - 2];
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::StrongStyle {
                            expected: expected_marker.to_string(),
                            found: found.to_string(),
                        },
                        self.default_severity(),
                    )
                    .with_range(range)
                    .with_fix(Fix::new(range, format!("{expected_marker}{inner}{expected_marker}"))),
                );
            }
        });
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["style"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        StrongStyle.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_consistent_asterisks() {
        assert!(run("**one** and **two**\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_marker() {
        let diagnostics = run("**one** and __two__\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "**two**");
    }

    #[test]
    fn flags_and_fixes_an_inconsistent_marker_after_multi_byte_text() {
        let diagnostics = run("従うように **one** and __two__\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "**two**");
    }
}
