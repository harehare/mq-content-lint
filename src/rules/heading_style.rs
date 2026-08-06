//! MD003: heading style (ATX `# Title`, closed ATX `# Title #`, or setext `Title\n=====`)
//! should be consistent across the document.
//!
//! `[rules.heading_style]` accepts a `style` key: `"consistent"` (default — match the first
//! heading found), `"atx"`, `"atx_closed"`, or `"setext"`. Only ATX <-> closed-ATX has a
//! mechanical fix; converting to/from setext would require rewriting a second line, which this
//! rule flags but does not auto-fix.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct HeadingStyle;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Atx,
    AtxClosed,
    Setext,
}

impl Style {
    fn as_str(self) -> &'static str {
        match self {
            Style::Atx => "atx",
            Style::AtxClosed => "atx_closed",
            Style::Setext => "setext",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "atx" => Some(Style::Atx),
            "atx_closed" => Some(Style::AtxClosed),
            "setext" => Some(Style::Setext),
            _ => None,
        }
    }
}

fn detect_style(line: &str) -> Style {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return Style::Setext;
    }
    let after_hashes = trimmed.trim_start_matches('#');
    if after_hashes.trim_end().ends_with('#') {
        Style::AtxClosed
    } else {
        Style::Atx
    }
}

impl Rule for HeadingStyle {
    fn id(&self) -> RuleId {
        RuleId::HeadingStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let configured = options.get_str("style").and_then(Style::parse);

        let mut diagnostics = Vec::new();
        let mut expected: Option<Style> = configured;
        let lines = crate::text::LineIndex::new(source);

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Heading(heading) = node else { return };
            let Some(position) = &heading.position else { return };
            let Some(line_text) = lines.get(position.start.line) else {
                return;
            };

            let found = detect_style(line_text);
            let expected_style = *expected.get_or_insert(found);

            if found != expected_style {
                let mut diagnostic = Diagnostic::new(
                    LintMessage::HeadingStyle {
                        expected: expected_style.as_str().to_string(),
                        found: found.as_str().to_string(),
                    },
                    self.default_severity(),
                )
                .with_range(position.clone());

                if let Some(fix) = rewrite(line_text, position.start.line, heading.depth, found, expected_style) {
                    diagnostic = diagnostic.with_fix(fix);
                }
                diagnostics.push(diagnostic);
            }
        });

        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["style"]
    }
}

/// Only ATX <-> closed-ATX is a single-line, unambiguous rewrite.
fn rewrite(line: &str, line_number: usize, depth: u8, found: Style, expected: Style) -> Option<Fix> {
    let text = match found {
        Style::Atx => line.trim_start().trim_start_matches('#').trim(),
        Style::AtxClosed => line
            .trim_start()
            .trim_start_matches('#')
            .trim()
            .trim_end_matches('#')
            .trim(),
        Style::Setext => return None,
    };
    if expected == Style::Setext {
        return None;
    }

    let hashes = "#".repeat(depth as usize);
    let replacement = match expected {
        Style::Atx => format!("{hashes} {text}"),
        Style::AtxClosed => format!("{hashes} {text} {hashes}"),
        Style::Setext => unreachable!(),
    };
    Some(Fix::new(
        Range::single_line(line_number, 1, line.chars().count() + 1),
        replacement,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        HeadingStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_when_all_atx() {
        assert!(run("# One\n\n## Two\n").is_empty());
    }

    #[test]
    fn flags_a_style_change_from_the_first_headings_style() {
        let diagnostics = run("# One\n\n## Two ##\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::HeadingStyle {
                expected: "atx".to_string(),
                found: "atx_closed".to_string(),
            }
        );
    }

    #[test]
    fn fix_rewrites_atx_closed_to_atx() {
        let diagnostics = run("# One\n\n## Two ##\n");
        let fix = diagnostics[0].fix.as_ref().unwrap();
        assert_eq!(fix.replacement, "## Two");
    }

    #[test]
    fn respects_configured_style() {
        let config = LintConfig::from_toml_str("[rules.heading_style]\nstyle = \"atx_closed\"\n").unwrap();
        let doc: mq_markdown::Markdown = "# One\n".parse().unwrap();
        let diagnostics = HeadingStyle.check(&doc, "# One\n", &config);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "# One #");
    }
}
