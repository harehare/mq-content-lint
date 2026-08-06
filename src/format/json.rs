use std::io::{self, Write};

use mq_content_lint::report_item::ReportItem;

/// Writes a single JSON array, one element per linted file, each carrying that file's
/// diagnostics (built-in and custom-rule alike). Field names are stable across releases within a
/// major version. `selector` is `null` for a custom rule (and for the handful of built-ins with
/// no single corresponding mq selector) — there's no other field distinguishing a custom rule's
/// diagnostic from a built-in's; matching `ruleId` against `mq-content-lint --list-rules`'
/// output is the way to tell.
pub(super) fn write_json_report(w: &mut impl Write, results: &[(String, Vec<ReportItem>)]) -> io::Result<()> {
    let report: Vec<serde_json::Value> = results
        .iter()
        .map(|(file_label, items)| {
            let diagnostics: Vec<serde_json::Value> = items
                .iter()
                .map(|item| {
                    let range = item.range().map(|r| {
                        serde_json::json!({
                            "startLine": r.start_line,
                            "startColumn": r.start_column,
                            "endLine": r.end_line,
                            "endColumn": r.end_column,
                        })
                    });

                    serde_json::json!({
                        "ruleId": item.rule_id(),
                        "selector": item.selector().map(|s| s.to_string()),
                        "severity": item.severity().to_string(),
                        "message": item.text(),
                        "help": item.help(),
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
        let source = "![](missing-alt.png)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let items: Vec<ReportItem> = Linter::with_default_rules()
            .run(&doc, source, &LintConfig::default())
            .into_iter()
            .map(ReportItem::from)
            .collect();
        let results = vec![("test.md".to_string(), items)];

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
        let results: Vec<(String, Vec<ReportItem>)> = vec![("test.md".to_string(), Vec::new())];
        let mut buf = Vec::new();
        write_json_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(json[0]["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_write_json_report_custom_rule_has_null_selector() {
        let item = ReportItem::Custom(mq_content_lint::custom_rules::CustomDiagnostic {
            rule_id: "no_todo".to_string(),
            message: "found a TODO".to_string(),
            severity: mq_content_lint::Severity::Warning,
            range: None,
            fix: None,
        });
        let results = vec![("test.md".to_string(), vec![item])];
        let mut buf = Vec::new();
        write_json_report(&mut buf, &results).unwrap();
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        let diag = &json[0]["diagnostics"][0];
        assert_eq!(diag["ruleId"], "no_todo");
        assert!(diag["selector"].is_null());
        assert_eq!(diag["message"], "found a TODO");
    }
}
