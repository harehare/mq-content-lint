//! MD013: line length. Configurable via `[rules.line_length] limit` (default 80, falling back to
//! a project's `.editorconfig` `max_line_length` if set and not overridden here) and
//! `code_blocks` (default `true` — whether fenced code block lines are checked too). Not
//! auto-fixable — safely rewrapping prose without breaking Markdown syntax needs judgment this
//! rule doesn't have.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct LineLength;

const DEFAULT_LIMIT: usize = 80;

impl Rule for LineLength {
    fn id(&self) -> RuleId {
        RuleId::LineLength
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
        let limit = options
            .get_usize("limit")
            .or(config.editorconfig_max_line_length)
            .unwrap_or(DEFAULT_LIMIT);
        let check_code_blocks = options.get_bool("code_blocks").unwrap_or(true);

        let code_ranges: Vec<(usize, usize)> = if check_code_blocks {
            Vec::new()
        } else {
            let mut ranges = Vec::new();
            crate::walk::walk(doc.nodes.iter(), &mut |node| {
                if let Node::Code(code) = node
                    && let Some(position) = &code.position
                {
                    ranges.push((position.start.line, position.end.line));
                }
            });
            ranges
        };
        let code_lines = crate::text::CodeBlockLines::new(code_ranges);

        crate::text::numbered_lines(source)
            .filter_map(|(line_number, line)| {
                if code_lines.contains(line_number) {
                    return None;
                }
                let length = line.chars().count();
                (length > limit).then(|| {
                    Diagnostic::new(LintMessage::LineLength { length, limit }, self.default_severity())
                        .with_range(Range::single_line(line_number, limit + 1, length + 1))
                })
            })
            .collect()
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["limit", "code_blocks"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with_limit(markdown: &str, limit: usize) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        let config = LintConfig::from_toml_str(&format!("[rules.line_length]\nlimit = {limit}\n")).unwrap();
        LineLength.check(&doc, markdown, &config, None)
    }

    #[test]
    fn no_diagnostics_under_the_limit() {
        assert!(run_with_limit("short line\n", 80).is_empty());
    }

    #[test]
    fn flags_a_line_over_the_limit() {
        let diagnostics = run_with_limit("this line is over the limit\n", 10);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::LineLength { length: 27, limit: 10 }
        );
    }

    #[test]
    fn falls_back_to_editorconfig_max_line_length_when_no_limit_is_configured() {
        let doc: mq_markdown::Markdown = "this line is over the limit\n".parse().unwrap();
        let mut config = LintConfig::default();
        config.editorconfig_max_line_length = Some(10);

        let diagnostics = LineLength.check(&doc, "this line is over the limit\n", &config, None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::LineLength { length: 27, limit: 10 }
        );
    }

    #[test]
    fn an_explicit_limit_wins_over_editorconfig_max_line_length() {
        let doc: mq_markdown::Markdown = "short\n".parse().unwrap();
        let mut config = LintConfig::from_toml_str("[rules.line_length]\nlimit = 3\n").unwrap();
        config.editorconfig_max_line_length = Some(80);

        let diagnostics = LineLength.check(&doc, "short\n", &config, None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, LintMessage::LineLength { length: 5, limit: 3 });
    }

    #[test]
    fn code_blocks_can_be_excluded() {
        let doc: mq_markdown::Markdown = "```\nthis is a very long line inside a code block\n```\n"
            .parse()
            .unwrap();
        let config = LintConfig::from_toml_str("[rules.line_length]\nlimit = 10\ncode_blocks = false\n").unwrap();
        let diagnostics = LineLength.check(
            &doc,
            "```\nthis is a very long line inside a code block\n```\n",
            &config,
            None,
        );
        assert!(diagnostics.is_empty());
    }
}
