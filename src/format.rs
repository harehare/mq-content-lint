//! Diagnostic output formats for the `mq-content-lint` CLI.

mod json;
mod markdown;
mod rdjson;
mod sarif;
mod text;

use std::io::{self, Write};

use mq_content_lint::report_item::ReportItem;

/// Diagnostic output format.
#[derive(Clone, Copy, Debug, Default, PartialEq, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    /// Human-readable report grouped by severity (default)
    #[default]
    Text,
    /// A single JSON array of diagnostics, one file's worth per element with a `file` field
    Json,
    /// GitHub-flavored Markdown table, suitable for a PR description or comment
    Markdown,
    /// SARIF 2.1.0 JSON, for GitHub code scanning and other SARIF consumers
    Sarif,
    /// RDJSON, for piping into `reviewdog -f=rdjson` (e.g. `-reporter=github-pr-review` for
    /// inline PR comments)
    Rdjson,
}

/// Dispatches to the writer for the requested output format.
///
/// Each entry is `(file_label, source, diagnostics)`; `source` is only used by the `Text`
/// report, to render a snippet with a caret under each diagnostic's range.
pub(crate) fn write_report(
    w: &mut impl Write,
    format: OutputFormat,
    results: &[(String, String, Vec<ReportItem>)],
) -> io::Result<()> {
    match format {
        OutputFormat::Text => {
            for (file_label, source, items) in results {
                text::write_text_report(w, file_label, source, items)?;
            }
            Ok(())
        }
        OutputFormat::Json => json::write_json_report(w, results),
        OutputFormat::Markdown => markdown::write_markdown_report(w, results),
        OutputFormat::Sarif => sarif::write_sarif_report(w, results),
        OutputFormat::Rdjson => rdjson::write_rdjson_report(w, results),
    }
}

/// Appends a Markdown diagnostics table to the GitHub Actions job summary, if
/// `GITHUB_STEP_SUMMARY` is set. No-op outside GitHub Actions or when `format` is `Markdown`
/// (the caller likely handles that report itself).
pub(crate) fn write_github_summary(
    format: OutputFormat,
    results: &[(String, String, Vec<ReportItem>)],
) -> io::Result<()> {
    if format == OutputFormat::Markdown {
        return Ok(());
    }
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return Ok(());
    };
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    markdown::write_markdown_report(&mut file, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results() -> Vec<(String, String, Vec<ReportItem>)> {
        let source = "![](missing-alt.png)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let items = mq_content_lint::Linter::with_default_rules()
            .run(&doc, source, &mq_content_lint::LintConfig::default(), None)
            .into_iter()
            .filter(|d| d.rule_id() == mq_content_lint::RuleId::ImageMissingAlt)
            .map(ReportItem::from)
            .collect();
        vec![("test.md".to_string(), source.to_string(), items)]
    }

    // One test, to avoid parallel tests racing on the shared GITHUB_STEP_SUMMARY env var.
    #[test]
    fn test_write_github_summary() {
        let path = std::env::temp_dir().join(format!("mq-content-lint-github-summary-test-{}.md", std::process::id()));

        // No env var: no-op.
        // SAFETY: sole test touching this env var.
        unsafe { std::env::remove_var("GITHUB_STEP_SUMMARY") };
        write_github_summary(OutputFormat::Text, &sample_results()).unwrap();

        // Env var set, non-markdown format: appends the table.
        std::fs::write(&path, "existing content\n").unwrap();
        // SAFETY: see above.
        unsafe { std::env::set_var("GITHUB_STEP_SUMMARY", &path) };
        write_github_summary(OutputFormat::Text, &sample_results()).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("existing content\n"));
        assert!(contents.contains("# mq-content-lint Report"));
        assert!(contents.contains("`image_missing_alt`"));

        // Markdown format: skipped.
        std::fs::write(&path, "existing content\n").unwrap();
        write_github_summary(OutputFormat::Markdown, &sample_results()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing content\n");

        // SAFETY: see above.
        unsafe { std::env::remove_var("GITHUB_STEP_SUMMARY") };
        std::fs::remove_file(&path).ok();
    }
}
