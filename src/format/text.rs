use std::io::{self, Write};

use colored::Colorize;
use mq_content_lint::Severity;

use mq_content_lint::report_item::ReportItem;

/// Severities in the order categories are displayed, most severe first.
const SEVERITY_ORDER: [Severity; 3] = [Severity::Error, Severity::Warning, Severity::Info];

/// Writes `items` grouped by severity in a Credo-style report and returns `true` if any were
/// reported. `code` is the source text `items` was computed against — used to render a snippet
/// with a caret under each diagnostic's range.
pub(super) fn write_text_report(
    w: &mut impl Write,
    file_label: &str,
    code: &str,
    items: &[ReportItem],
) -> io::Result<bool> {
    let mut printed_category = false;
    for severity in SEVERITY_ORDER {
        let group: Vec<&ReportItem> = items.iter().filter(|d| d.severity() == severity).collect();
        if group.is_empty() {
            continue;
        }
        if printed_category {
            writeln!(w)?;
        }
        printed_category = true;
        write_category(w, severity, &group, file_label, code)?;
    }

    if items.is_empty() {
        writeln!(
            w,
            "{}  {}",
            "✓".bright_green().bold(),
            "No content lint issues found.".bright_green()
        )?;
    } else {
        writeln!(w)?;
        write_summary(w, items)?;
    }

    Ok(!items.is_empty())
}

/// Maps a severity to its category title and one-letter marker.
fn severity_category(severity: Severity) -> (colored::ColoredString, colored::ColoredString) {
    match severity {
        Severity::Error => ("Errors".bright_red().bold(), "[E]".bright_red().bold()),
        Severity::Warning => ("Warnings".bright_yellow().bold(), "[W]".bright_yellow().bold()),
        Severity::Info => ("Info".blue().bold(), "[I]".blue().bold()),
    }
}

/// Colors `s` to match `severity`, shared by the box frame, gutter bar, message text, and the
/// snippet's caret.
fn severity_color(severity: Severity, s: &str) -> colored::ColoredString {
    match severity {
        Severity::Error => s.bright_red(),
        Severity::Warning => s.bright_yellow(),
        Severity::Info => s.blue(),
    }
}

/// Writes one severity category as a box-drawn frame (`┌─ Title` … `└─`) around its
/// diagnostics, each as a `[X] message` line, a source snippet with a caret underline (when a
/// range is known), then the `file:line:col rule_id` location (a built-in rule's id is followed
/// by the mq selector it corresponds to, e.g. `image_missing_alt (.image)`; a custom rule just
/// shows its configured id).
fn write_category(
    w: &mut impl Write,
    severity: Severity,
    items: &[&ReportItem],
    file_label: &str,
    code: &str,
) -> io::Result<()> {
    let (title, letter) = severity_category(severity);
    let bar = severity_color(severity, "│");

    writeln!(w, "{} {title}", severity_color(severity, "┌─").bold())?;
    writeln!(w, "{bar}")?;

    for (i, item) in items.iter().enumerate() {
        writeln!(
            w,
            "{bar} {} {}",
            letter,
            severity_color(item.severity(), &item.text()).bold()
        )?;

        if let Some(range) = item.range() {
            writeln!(w, "{bar}")?;
            write_snippet(w, code, &range, severity, &bar)?;
        }

        let loc = match item.range() {
            Some(range) => format!("{}:{}:{}", file_label, range.start_line, range.start_column),
            None => file_label.to_string(),
        };
        let rule_label = match item.selector() {
            Some(selector) => format!("{} ({})", item.rule_id(), selector),
            None => item.rule_id().to_string(),
        };
        writeln!(w, "{bar}     {} {}", loc.dimmed(), rule_label.dimmed())?;

        if let Some(help) = item.help() {
            writeln!(w, "{bar}       {}", format!("help: {help}").bright_blue())?;
        }

        if i + 1 < items.len() {
            writeln!(w, "{bar}")?;
        }
    }

    writeln!(w, "{}", severity_color(severity, "└─"))?;

    Ok(())
}

/// Writes the offending source line with a caret underline beneath it, colored to match
/// `severity` and indented under the category's gutter `bar`.
fn write_snippet(
    w: &mut impl Write,
    code: &str,
    range: &mq_content_lint::Range,
    severity: Severity,
    bar: &colored::ColoredString,
) -> io::Result<()> {
    let lines: Vec<&str> = code.lines().collect();
    let line_idx = range.start_line.saturating_sub(1);
    let Some(source_line) = lines.get(line_idx) else {
        return Ok(());
    };

    let col_start = range.start_column.saturating_sub(1);
    let col_end = if range.end_line == range.start_line {
        range.end_column.saturating_sub(1)
    } else {
        source_line.len()
    };
    let underline_len = col_end.saturating_sub(col_start).max(1);
    let line_num = range.start_line.to_string();

    writeln!(w, "{bar}    {} {} {}", line_num.dimmed(), "│".dimmed(), source_line)?;
    writeln!(
        w,
        "{bar}    {} {} {}{}",
        " ".repeat(line_num.len()),
        "│".dimmed(),
        " ".repeat(col_start),
        severity_color(severity, &"^".repeat(underline_len)).bold(),
    )?;
    writeln!(w, "{bar}")
}

/// Writes the trailing summary line, e.g. `found 3 issues (2 errors, 1 warning).`
fn write_summary(w: &mut impl Write, items: &[ReportItem]) -> io::Result<()> {
    let breakdown: Vec<String> = SEVERITY_ORDER
        .into_iter()
        .filter_map(|severity| {
            let count = items.iter().filter(|d| d.severity() == severity).count();
            if count == 0 {
                return None;
            }
            let (singular, plural) = match severity {
                Severity::Error => ("error".bright_red(), "errors".bright_red()),
                Severity::Warning => ("warning".bright_yellow(), "warnings".bright_yellow()),
                Severity::Info => ("info".blue(), "info".blue()),
            };
            Some(format!("{count} {}", if count == 1 { singular } else { plural }))
        })
        .collect();

    writeln!(
        w,
        "{} {} issue{} ({}).",
        "found".bold(),
        items.len().to_string().bold(),
        if items.len() == 1 { "" } else { "s" },
        breakdown.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    fn sample_source() -> &'static str {
        "![](missing-alt.png)\n"
    }

    fn sample_items() -> Vec<ReportItem> {
        let source = sample_source();
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let config = LintConfig::default();
        Linter::with_default_rules()
            .run(&doc, source, &config, None)
            .into_iter()
            .filter(|d| d.rule_id() == mq_content_lint::RuleId::ImageMissingAlt)
            .map(ReportItem::from)
            .collect()
    }

    #[test]
    fn test_write_text_report_no_issues() {
        let mut buf = Vec::new();
        let had_diagnostics = write_text_report(&mut buf, "test.md", "", &[]).unwrap();
        assert!(!had_diagnostics);
        assert!(
            String::from_utf8(buf)
                .unwrap()
                .contains("No content lint issues found.")
        );
    }

    #[test]
    fn test_write_text_report_with_diagnostics() {
        let items = sample_items();
        let mut buf = Vec::new();
        let had_diagnostics = write_text_report(&mut buf, "test.md", sample_source(), &items).unwrap();
        assert!(had_diagnostics);

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("┌─ Errors"));
        assert!(output.contains("└─"));
        assert!(output.contains("test.md:1:1"));
        assert!(output.contains("found 1 issue (1 error)."));
    }

    #[test]
    fn test_write_text_report_shows_snippet_with_caret() {
        let items = sample_items();
        let mut buf = Vec::new();
        write_text_report(&mut buf, "test.md", sample_source(), &items).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("![](missing-alt.png)"));
        assert!(output.contains('^'));
    }

    #[test]
    fn test_write_text_report_with_custom_rule() {
        let item = ReportItem::Custom(mq_content_lint::custom_rules::CustomDiagnostic {
            rule_id: "no_todo".to_string(),
            message: "found a TODO".to_string(),
            severity: Severity::Warning,
            range: None,
            fix: None,
        });
        let mut buf = Vec::new();
        write_text_report(&mut buf, "test.md", "", &[item]).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("found a TODO"));
        assert!(output.contains("no_todo"));
    }
}
