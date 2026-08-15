use std::io::{self, Write};

use mq_content_lint::report_item::ReportItem;

/// Writes a GitHub-flavored Markdown table of diagnostics across every linted file, suitable for
/// a PR description or comment.
pub(super) fn write_markdown_report(
    w: &mut impl Write,
    results: &[(String, String, Vec<ReportItem>)],
) -> io::Result<()> {
    let issue_count: usize = results.iter().map(|(_, _source, items)| items.len()).sum();

    writeln!(w, "# mq-content-lint Report")?;
    writeln!(w)?;

    if issue_count == 0 {
        writeln!(w, "No content lint issues found.")?;
        return Ok(());
    }

    writeln!(w, "| File | Severity | Rule | Location | Message |")?;
    writeln!(w, "| --- | --- | --- | --- | --- |")?;

    for (file_label, _source, items) in results {
        for item in items {
            let loc = match item.range() {
                Some(range) => format!("{}:{}", range.start_line, range.start_column),
                None => String::new(),
            };
            let mut message = escape_cell(&item.text());
            if let Some(help) = item.help() {
                message.push_str(" — help: ");
                message.push_str(&escape_cell(&help));
            }

            writeln!(
                w,
                "| {} | {} | `{}` | {} | {} |",
                escape_cell(file_label),
                item.severity(),
                item.rule_id(),
                loc,
                message,
            )?;
        }
    }

    writeln!(w)?;
    writeln!(
        w,
        "**Found {issue_count} issue{}.**",
        if issue_count == 1 { "" } else { "s" }
    )
}

/// Escapes pipes and newlines, which would otherwise break a Markdown table cell.
fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    fn sample_items() -> Vec<ReportItem> {
        let source = "![](missing-alt.png)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        Linter::with_default_rules()
            .run(&doc, source, &LintConfig::default(), None)
            .into_iter()
            .filter(|d| d.rule_id() == mq_content_lint::RuleId::ImageMissingAlt)
            .map(ReportItem::from)
            .collect()
    }

    #[test]
    fn test_write_markdown_report_produces_table() {
        let items = sample_items();
        assert!(!items.is_empty());
        let results = vec![("test.md".to_string(), "![](missing-alt.png)\n".to_string(), items)];

        let mut buf = Vec::new();
        write_markdown_report(&mut buf, &results).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("# mq-content-lint Report"));
        assert!(output.contains("| File | Severity | Rule | Location | Message |"));
        assert!(output.contains("test.md"));
        assert!(output.contains("`image_missing_alt`"));
        assert!(output.contains("**Found 1 issue.**"));
    }

    #[test]
    fn test_write_markdown_report_no_issues() {
        let results: Vec<(String, String, Vec<ReportItem>)> = vec![("test.md".to_string(), String::new(), Vec::new())];
        let mut buf = Vec::new();
        write_markdown_report(&mut buf, &results).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("No content lint issues found."));
        assert!(!output.contains('|'));
    }

    #[test]
    fn test_escape_cell() {
        assert_eq!(escape_cell("a | b"), "a \\| b");
        assert_eq!(escape_cell("line1\nline2"), "line1 line2");
    }
}
