//! MD054: link/image style consistency — `autolink` (`<https://url>`), `inline`
//! (`[text](url)`), `full` reference (`[text][label]`), `collapsed` reference (`[text][]`), or
//! `shortcut` reference (`[text]`). All styles are allowed by default (a no-op, like
//! [`crate::rules::missing_front_matter_key`] with no required keys); set any of
//! `[rules.link_image_style] autolink/inline/full/collapsed/shortcut = false` to disallow it.
//! Not auto-fixable — converting between styles can need information (a label, a URL) this rule
//! doesn't have.
//!
//! `mq_markdown::Position::column` counts UTF-8 bytes, not `char`s — see
//! [`super::no_space_in_code`]'s doc comment — so `position` is converted to a char-counted
//! `Range` through [`crate::text::LineByteIndex::char_column`] before it's passed to
//! [`crate::fix::slice`], or a link/image containing multi-byte text would misalign the raw
//! substring extracted for style detection.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct LinkImageStyle;

fn detect_style(raw: &str) -> &'static str {
    if raw.starts_with('<') {
        "autolink"
    } else if raw.ends_with("[]") {
        "collapsed"
    } else if raw.ends_with(']') && raw.matches('[').count() >= 2 {
        "full"
    } else if raw.ends_with(')') {
        "inline"
    } else {
        "shortcut"
    }
}

impl Rule for LinkImageStyle {
    fn id(&self) -> RuleId {
        RuleId::LinkImageStyle
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
        config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let disallowed: Vec<&str> = ["autolink", "inline", "full", "collapsed", "shortcut"]
            .into_iter()
            .filter(|style| options.get_bool(style) == Some(false))
            .collect();
        if disallowed.is_empty() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        let byte_index = crate::text::LineByteIndex::new(source);
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let position = match node {
                Node::Link(l) => l.position.clone(),
                Node::LinkRef(l) => l.position.clone(),
                Node::Image(i) => i.position.clone(),
                Node::ImageRef(i) => i.position.clone(),
                _ => return,
            };
            let Some(position) = position else { return };
            let Some(start_column) = byte_index.char_column(position.start.line, position.start.column) else {
                return;
            };
            let Some(end_column) = byte_index.char_column(position.end.line, position.end.column) else {
                return;
            };
            let range = Range {
                start_line: position.start.line,
                start_column,
                end_line: position.end.line,
                end_column,
            };
            let Some(raw) = crate::fix::slice(source, &byte_index, range) else {
                return;
            };
            let found = detect_style(raw);

            if disallowed.contains(&found) {
                let allowed: Vec<&str> = ["autolink", "inline", "full", "collapsed", "shortcut"]
                    .into_iter()
                    .filter(|s| !disallowed.contains(s))
                    .collect();
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::LinkImageStyle {
                            expected: allowed.join("/"),
                            found: found.to_string(),
                        },
                        self.default_severity(),
                    )
                    .with_range(position),
                );
            }
        });
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["autolink", "inline", "full", "collapsed", "shortcut"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_op_with_no_config() {
        let doc: mq_markdown::Markdown = "[text](url)\n".parse().unwrap();
        assert!(
            LinkImageStyle
                .check(&doc, "[text](url)\n", &LintConfig::default(), None)
                .is_empty()
        );
    }

    #[test]
    fn flags_a_disallowed_style() {
        let config = LintConfig::from_toml_str("[rules.link_image_style]\nautolink = false\n").unwrap();
        let source = "<https://example.com>\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let diagnostics = LinkImageStyle.check(&doc, source, &config, None);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::LinkImageStyle {
                expected: "inline/full/collapsed/shortcut".to_string(),
                found: "autolink".to_string(),
            }
        );
    }

    #[test]
    fn allowed_styles_are_not_flagged() {
        let config = LintConfig::from_toml_str("[rules.link_image_style]\nautolink = false\n").unwrap();
        let source = "[text](https://example.com)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        assert!(LinkImageStyle.check(&doc, source, &config, None).is_empty());
    }

    #[test]
    fn detects_a_disallowed_style_after_multi_byte_text() {
        let config = LintConfig::from_toml_str("[rules.link_image_style]\nautolink = false\n").unwrap();
        let source = "従うように <https://example.com>\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let diagnostics = LinkImageStyle.check(&doc, source, &config, None);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::LinkImageStyle {
                expected: "inline/full/collapsed/shortcut".to_string(),
                found: "autolink".to_string(),
            }
        );
    }
}
