use std::io::{self, Write};

use colored::Colorize;
use mq_content_lint::{Diagnostic, Severity};

/// Severities in the order categories are displayed, most severe first.
const SEVERITY_ORDER: [Severity; 3] = [Severity::Error, Severity::Warning, Severity::Info];

/// Writes `diagnostics` grouped by severity and returns `true` if any were reported.
pub(super) fn write_text_report(w: &mut impl Write, file_label: &str, diagnostics: &[Diagnostic]) -> io::Result<bool> {
    let mut printed_category = false;
    for severity in SEVERITY_ORDER {
        let group: Vec<&Diagnostic> = diagnostics.iter().filter(|d| d.severity == severity).collect();
        if group.is_empty() {
            continue;
        }
        if printed_category {
            writeln!(w)?;
        }
        printed_category = true;
        write_category(w, severity, &group, file_label)?;
    }

    if diagnostics.is_empty() {
        writeln!(
            w,
            "{}  {}",
            "✓".bright_green().bold(),
            "No content lint issues found.".bright_green()
        )?;
    } else {
        writeln!(w)?;
        write_summary(w, diagnostics)?;
    }

    Ok(!diagnostics.is_empty())
}

/// Maps a severity to its category title and one-letter marker.
fn severity_category(severity: Severity) -> (colored::ColoredString, colored::ColoredString) {
    match severity {
        Severity::Error => ("## Errors".bright_red().bold(), "[E]".bright_red().bold()),
        Severity::Warning => ("## Warnings".bright_yellow().bold(), "[W]".bright_yellow().bold()),
        Severity::Info => ("## Info".blue().bold(), "[I]".blue().bold()),
    }
}

fn severity_bar(severity: Severity) -> colored::ColoredString {
    match severity {
        Severity::Error => "|".bright_red(),
        Severity::Warning => "|".bright_yellow(),
        Severity::Info => "|".blue(),
    }
}

/// Writes one severity category: a heading followed by its diagnostics, each as a
/// `[X] message` line with the `file:line:col .rule_id` location on the line below (the rule
/// id rendered as the mq selector it corresponds to, e.g. `.image` for `image_missing_alt`).
fn write_category(
    w: &mut impl Write,
    severity: Severity,
    diagnostics: &[&Diagnostic],
    file_label: &str,
) -> io::Result<()> {
    let (title, letter) = severity_category(severity);
    let bar = severity_bar(severity);

    writeln!(w, "{}\n", title)?;

    for (i, diagnostic) in diagnostics.iter().enumerate() {
        match diagnostic.severity {
            Severity::Error => writeln!(w, "{bar} {} {}", letter, diagnostic.text().bright_red().bold())?,
            Severity::Warning => writeln!(w, "{bar} {} {}", letter, diagnostic.text().bright_yellow().bold())?,
            Severity::Info => writeln!(w, "{bar} {} {}", letter, diagnostic.text().blue().bold())?,
        }

        let loc = match &diagnostic.range {
            Some(range) => format!("{}:{}:{}", file_label, range.start_line, range.start_column),
            None => file_label.to_string(),
        };
        let rule_label = match diagnostic.rule_id().selector() {
            Some(selector) => format!("{} ({})", diagnostic.rule_id(), selector),
            None => diagnostic.rule_id().to_string(),
        };
        writeln!(w, "{bar}     {} {}", loc.dimmed(), rule_label.dimmed())?;

        if let Some(help) = diagnostic.help() {
            writeln!(w, "{bar}       {}", format!("help: {help}").bright_blue())?;
        }

        if i + 1 < diagnostics.len() {
            writeln!(w, "{bar}")?;
        }
    }

    Ok(())
}

/// Writes the trailing summary line, e.g. `found 3 issues (2 errors, 1 warning).`
fn write_summary(w: &mut impl Write, diagnostics: &[Diagnostic]) -> io::Result<()> {
    let breakdown: Vec<String> = SEVERITY_ORDER
        .into_iter()
        .filter_map(|severity| {
            let count = diagnostics.iter().filter(|d| d.severity == severity).count();
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
        diagnostics.len().to_string().bold(),
        if diagnostics.len() == 1 { "" } else { "s" },
        breakdown.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    fn sample_diagnostics() -> Vec<Diagnostic> {
        let source = "![](missing-alt.png)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let config = LintConfig::default();
        Linter::with_default_rules()
            .run(&doc, source, &config)
            .into_iter()
            .filter(|d| d.rule_id() == mq_content_lint::RuleId::ImageMissingAlt)
            .collect()
    }

    #[test]
    fn test_write_text_report_no_issues() {
        let mut buf = Vec::new();
        let had_diagnostics = write_text_report(&mut buf, "test.md", &[]).unwrap();
        assert!(!had_diagnostics);
        assert!(
            String::from_utf8(buf)
                .unwrap()
                .contains("No content lint issues found.")
        );
    }

    #[test]
    fn test_write_text_report_with_diagnostics() {
        let diagnostics = sample_diagnostics();
        let mut buf = Vec::new();
        let had_diagnostics = write_text_report(&mut buf, "test.md", &diagnostics).unwrap();
        assert!(had_diagnostics);

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("## Errors"));
        assert!(output.contains("test.md:1:1"));
        assert!(output.contains("found 1 issue (1 error)."));
    }
}
