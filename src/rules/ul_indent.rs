//! MD007: an unordered sub-list item should be indented by a fixed number of spaces per nesting
//! level (`[rules.ul_indent] indent`, default 2), relative to the top level at 0.
//!
//! Uses the raw line's leading whitespace, not `Position::start.column` — see
//! [`super::list_indent`]'s docs for why.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct UlIndent;

const DEFAULT_INDENT: usize = 2;

impl Rule for UlIndent {
    fn id(&self) -> RuleId {
        RuleId::UlIndent
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
        let spaces = config
            .rule_options(self.id())
            .get_usize("indent")
            .unwrap_or(DEFAULT_INDENT);
        let mut diagnostics = Vec::new();
        let lines = crate::text::LineIndex::new(source);

        for node in &doc.nodes {
            let Node::List(list) = node else { continue };
            if list.ordered {
                continue;
            }
            let Some(position) = &list.position else { continue };
            let Some(line) = lines.get(position.start.line) else {
                continue;
            };
            let indent = line.len() - line.trim_start().len();
            let expected = list.level as usize * spaces;

            if indent != expected {
                let indent_range = Range::single_line(position.start.line, 1, indent + 1);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::UlIndent {
                            expected,
                            found: indent,
                        },
                        self.default_severity(),
                    )
                    .with_range(indent_range)
                    .with_fix(Fix::new(indent_range, " ".repeat(expected))),
                );
            }
        }
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["indent"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        UlIndent.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_top_level_items() {
        assert!(run("- one\n- two\n").is_empty());
    }

    #[test]
    fn respects_configured_indent_width() {
        let config = LintConfig::from_toml_str("[rules.ul_indent]\nindent = 4\n").unwrap();
        let doc: mq_markdown::Markdown = "- one\n".parse().unwrap();
        assert!(UlIndent.check(&doc, "- one\n", &config, None).is_empty());
    }
}
