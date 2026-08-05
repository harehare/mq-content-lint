use std::io::{self, Write};

use mq_content_lint::{Diagnostic, Severity};

/// Writes a single SARIF 2.1.0 log document covering every linted file.
///
/// See <https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/sarif-v2.1.0-os.html>.
pub(super) fn write_sarif_report(w: &mut impl Write, results: &[(String, Vec<Diagnostic>)]) -> io::Result<()> {
    let sarif_results: Vec<serde_json::Value> = results
        .iter()
        .flat_map(|(file_label, diagnostics)| {
            diagnostics.iter().map(move |diagnostic| {
                let mut physical_location = serde_json::json!({
                    "artifactLocation": {"uri": file_label},
                });
                if let Some(range) = &diagnostic.range {
                    physical_location["region"] = serde_json::json!({
                        "startLine": range.start_line,
                        "startColumn": range.start_column,
                        "endLine": range.end_line,
                        "endColumn": range.end_column,
                    });
                }

                serde_json::json!({
                    "ruleId": diagnostic.rule_id().as_str(),
                    "level": sarif_level(diagnostic.severity),
                    "message": {"text": diagnostic.text()},
                    "locations": [{"physicalLocation": physical_location}],
                })
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "mq-content-lint",
                    "informationUri": "https://github.com/harehare/mq-content-lint",
                    "version": env!("CARGO_PKG_VERSION"),
                    "rules": mq_content_lint::RuleId::ALL.iter().map(|id| serde_json::json!({
                        "id": id.as_str(),
                        "name": id.as_str(),
                    })).collect::<Vec<_>>(),
                }
            },
            "results": sarif_results,
        }],
    });

    writeln!(w, "{}", serde_json::to_string_pretty(&sarif).map_err(io::Error::other)?)
}

/// Maps a lint [`Severity`] to a SARIF result `level` (`error`, `warning`, or `note`).
fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    fn sample_diagnostics() -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = "![](missing-alt.png)\n".parse().unwrap();
        Linter::with_default_rules().run(&doc, &LintConfig::default())
    }

    #[test]
    fn test_write_sarif_report_produces_valid_sarif_shape() {
        let diagnostics = sample_diagnostics();
        assert!(!diagnostics.is_empty());
        let results = vec![("test.md".to_string(), diagnostics)];

        let mut buf = Vec::new();
        write_sarif_report(&mut buf, &results).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["version"], "2.1.0");
        assert_eq!(json["runs"][0]["tool"]["driver"]["name"], "mq-content-lint");
        let result = &json["runs"][0]["results"][0];
        assert_eq!(result["ruleId"], "image_missing_alt");
        assert_eq!(result["level"], "error");
        assert_eq!(
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "test.md"
        );
        assert_eq!(result["locations"][0]["physicalLocation"]["region"]["startLine"], 1);
    }

    #[test]
    fn test_write_sarif_report_empty_diagnostics() {
        let results = vec![("test.md".to_string(), Vec::new())];
        let mut buf = Vec::new();
        write_sarif_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(json["runs"][0]["results"].as_array().unwrap().len(), 0);
    }
}
