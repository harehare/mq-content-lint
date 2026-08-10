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
