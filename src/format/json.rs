use std::io::{self, Write};

use mq_content_lint::Diagnostic;

/// Writes a single JSON array, one element per linted file, each carrying that file's
/// diagnostics. Rule ids and field names are stable across releases within a major version.
pub(super) fn write_json_report(w: &mut impl Write, results: &[(String, Vec<Diagnostic>)]) -> io::Result<()> {
    let report: Vec<serde_json::Value> = results
        .iter()
        .map(|(file_label, diagnostics)| {
            let diagnostics: Vec<serde_json::Value> = diagnostics
                .iter()
                .map(|diagnostic| {
                    let range = diagnostic.range.map(|r| {
                        serde_json::json!({
                            "startLine": r.start_line,
                            "startColumn": r.start_column,
                            "endLine": r.end_line,
                            "endColumn": r.end_column,
                        })
                    });

                    serde_json::json!({
                        "ruleId": diagnostic.rule_id().as_str(),
                        "selector": diagnostic.rule_id().selector().to_string(),
                        "severity": diagnostic.severity.to_string(),
                        "message": diagnostic.text(),
                        "help": diagnostic.help(),
                        "range": range,
                    })
                })
                .collect();

            serde_json::json!({
                "file": file_label,
                "diagnostics": diagnostics,
            })
        })
        .collect();

    writeln!(
        w,
        "{}",
        serde_json::to_string_pretty(&report).map_err(io::Error::other)?
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    #[test]
    fn test_write_json_report_shape() {
        let doc: mq_markdown::Markdown = "![](missing-alt.png)\n".parse().unwrap();
        let diagnostics = Linter::with_default_rules().run(&doc, &LintConfig::default());
        let results = vec![("test.md".to_string(), diagnostics)];

        let mut buf = Vec::new();
        write_json_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();

        assert_eq!(json[0]["file"], "test.md");
        let diag = &json[0]["diagnostics"][0];
        assert_eq!(diag["ruleId"], "image_missing_alt");
        assert_eq!(diag["selector"], ".image");
        assert_eq!(diag["severity"], "error");
        assert_eq!(diag["range"]["startLine"], 1);
    }

    #[test]
    fn test_write_json_report_empty_diagnostics() {
        let results = vec![("test.md".to_string(), Vec::new())];
        let mut buf = Vec::new();
        write_json_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(json[0]["diagnostics"].as_array().unwrap().len(), 0);
    }
}
