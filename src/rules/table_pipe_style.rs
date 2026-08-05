//! MD055: table row pipe style (leading/trailing `|`) should be consistent across the document.
//! `[rules.table_pipe_style] style` accepts `"consistent"` (default — match the first table row
//! found), `"leading_and_trailing"`, `"leading_only"`, `"trailing_only"`, or `"no_leading_or_trailing"`.

use std::collections::HashSet;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct TablePipeStyle;

fn detect_style(line: &str) -> &'static str {
    let trimmed = line.trim();
    match (trimmed.starts_with('|'), trimmed.ends_with('|')) {
        (true, true) => "leading_and_trailing",
        (true, false) => "leading_only",
        (false, true) => "trailing_only",
        (false, false) => "no_leading_or_trailing",
    }
}

fn rewrite(line: &str, style: &str) -> String {
    let trimmed = line.trim();
    let core = trimmed.trim_start_matches('|').trim_end_matches('|').trim();
    match style {
        "leading_and_trailing" => format!("| {core} |"),
        "leading_only" => format!("| {core}"),
        "trailing_only" => format!("{core} |"),
        _ => core.to_string(),
    }
}

impl Rule for TablePipeStyle {
    fn id(&self) -> RuleId {
        RuleId::TablePipeStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let mut expected = config.rule_options(self.id()).get_str("style").map(str::to_string);
        let mut seen_lines = HashSet::new();
        let mut diagnostics = Vec::new();

        for node in &doc.nodes {
            let position = match node {
                Node::TableCell(c) => &c.position,
                Node::TableAlign(a) => &a.position,
                _ => continue,
            };
            let Some(position) = position else { continue };
            if !seen_lines.insert(position.start.line) {
                continue;
            }
            let Some((_, line)) = crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line) else {
                continue;
            };
            let found = detect_style(line);
            let expected_style = expected.get_or_insert_with(|| found.to_string()).clone();

            if found != expected_style {
                let range = Range::single_line(position.start.line, 1, line.chars().count() + 1);
                diagnostics.push(
                    Diagnostic::new(
                        LintMessage::TablePipeStyle {
                            expected: expected_style.clone(),
                        },
                        self.default_severity(),
                    )
                    .with_range(range)
                    .with_fix(Fix::new(range, rewrite(line, &expected_style))),
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
        TablePipeStyle.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_consistent_pipes() {
        assert!(run("| A | B |\n|---|---|\n| 1 | 2 |\n").is_empty());
    }

    #[test]
    fn flags_an_inconsistent_row() {
        let diagnostics = run("| A | B |\n|---|---|\nA | B\n");
        assert!(!diagnostics.is_empty());
    }
}
