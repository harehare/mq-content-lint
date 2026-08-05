//! Diagnostic output formats for the `mq-content-lint` CLI.

mod json;
mod sarif;
mod text;

use std::io::{self, Write};

use mq_content_lint::Diagnostic;

/// Diagnostic output format.
#[derive(Clone, Copy, Debug, Default, PartialEq, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    /// Human-readable report grouped by severity (default)
    #[default]
    Text,
    /// A single JSON array of diagnostics, one file's worth per element with a `file` field
    Json,
    /// SARIF 2.1.0 JSON, for GitHub code scanning and other SARIF consumers
    Sarif,
}

/// Dispatches to the writer for the requested output format.
pub(crate) fn write_report(
    w: &mut impl Write,
    format: OutputFormat,
    results: &[(String, Vec<Diagnostic>)],
) -> io::Result<()> {
    match format {
        OutputFormat::Text => {
            for (file_label, diagnostics) in results {
                text::write_text_report(w, file_label, diagnostics)?;
            }
            Ok(())
        }
        OutputFormat::Json => json::write_json_report(w, results),
        OutputFormat::Sarif => sarif::write_sarif_report(w, results),
    }
}
