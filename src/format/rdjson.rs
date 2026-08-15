use std::io::{self, Write};

use mq_content_lint::Severity;
use mq_content_lint::report_item::ReportItem;

/// Writes a single [RDJSON](https://github.com/reviewdog/reviewdog/tree/master/proto/rdf) document
/// — the format [reviewdog](https://github.com/reviewdog/reviewdog) reads with `-f=rdjson`, to
/// post diagnostics as inline PR review comments (`-reporter=github-pr-review`) instead of a
/// flat CI log. A diagnostic with a [`mq_content_lint::Fix`] carries it as an RDJSON
/// `suggestion`, which reviewdog can offer as a one-click GitHub suggested change.
pub(super) fn write_rdjson_report(w: &mut impl Write, results: &[(String, String, Vec<ReportItem>)]) -> io::Result<()> {
    let diagnostics: Vec<serde_json::Value> = results
        .iter()
        .flat_map(|(file_label, _source, items)| {
            items.iter().map(move |item| {
                let mut location = serde_json::json!({"path": file_label});
                if let Some(range) = item.range() {
                    location["range"] = rdjson_range(range);
                }

                let mut diagnostic = serde_json::json!({
                    "message": item.text(),
                    "location": location,
                    "severity": rdjson_severity(item.severity()),
                    "source": {"name": "mq-content-lint"},
                    "code": {"value": item.rule_id()},
                });
                if let Some(fix) = item.fix() {
                    diagnostic["suggestions"] = serde_json::json!([{
                        "range": rdjson_range(fix.range),
                        "text": fix.replacement,
                    }]);
                }
                diagnostic
            })
        })
        .collect();

    let rdjson = serde_json::json!({
        "source": {
            "name": "mq-content-lint",
            "url": "https://github.com/harehare/mq-content-lint",
        },
        "severity": "UNKNOWN_SEVERITY",
        "diagnostics": diagnostics,
    });

    writeln!(
        w,
        "{}",
        serde_json::to_string_pretty(&rdjson).map_err(io::Error::other)?
    )
}

fn rdjson_range(range: mq_content_lint::Range) -> serde_json::Value {
    serde_json::json!({
        "start": {"line": range.start_line, "column": range.start_column},
        "end": {"line": range.end_line, "column": range.end_column},
    })
}

/// Maps a lint [`Severity`] to an RDJSON `severity` (`ERROR`, `WARNING`, or `INFO`).
fn rdjson_severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "ERROR",
        Severity::Warning => "WARNING",
        Severity::Info => "INFO",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{LintConfig, Linter};

    fn sample_items() -> Vec<ReportItem> {
        let source = "#Title\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        Linter::with_default_rules()
            .run(&doc, source, &LintConfig::default(), None)
            .into_iter()
            .map(ReportItem::from)
            .collect()
    }

    #[test]
    fn test_write_rdjson_report_produces_valid_shape() {
        let items = sample_items();
        assert!(!items.is_empty());
        let results = vec![("test.md".to_string(), "#Title\n".to_string(), items)];

        let mut buf = Vec::new();
        write_rdjson_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();

        assert_eq!(json["source"]["name"], "mq-content-lint");
        let diagnostics = json["diagnostics"].as_array().unwrap();
        let diag = diagnostics
            .iter()
            .find(|d| d["code"]["value"] == "no_missing_space_atx")
            .expect("no_missing_space_atx should have fired");
        assert_eq!(diag["severity"], "WARNING");
        assert_eq!(diag["location"]["path"], "test.md");
        assert_eq!(diag["location"]["range"]["start"]["line"], 1);
        // no_missing_space_atx always has a fix (inserting the missing space), so it should come
        // through as a suggestion.
        assert_eq!(diag["suggestions"][0]["text"], " ");
    }

    #[test]
    fn test_write_rdjson_report_omits_suggestions_when_there_is_no_fix() {
        let item = ReportItem::Custom(mq_content_lint::custom_rules::CustomDiagnostic {
            rule_id: "no_todo".to_string(),
            message: "found a TODO".to_string(),
            severity: Severity::Warning,
            range: None,
            fix: None,
        });
        let results = vec![("test.md".to_string(), String::new(), vec![item])];

        let mut buf = Vec::new();
        write_rdjson_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();

        let diag = &json["diagnostics"][0];
        assert_eq!(diag["code"]["value"], "no_todo");
        assert!(diag["suggestions"].is_null());
        assert!(diag["location"]["range"].is_null());
    }

    #[test]
    fn test_write_rdjson_report_empty_diagnostics() {
        let results: Vec<(String, String, Vec<ReportItem>)> = vec![("test.md".to_string(), String::new(), Vec::new())];
        let mut buf = Vec::new();
        write_rdjson_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(json["diagnostics"].as_array().unwrap().len(), 0);
    }
}
