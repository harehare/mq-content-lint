//! User-defined lint rules expressed as mq queries.
//!
//! The crate-level docs call this out as a later stage of the project, deferred behind the 53
//! deterministic built-in rules — this is that stage. A [`CustomRule`]'s `query` is evaluated
//! against the document with mq's own query engine (the same one `mq` and `mq-lint` embed);
//! every result that resolves to a markdown node with a source position becomes one diagnostic
//! at that position, using the rule's configured `message` and `severity`. This is the one
//! capability neither `markdownlint`/`markdownlint-cli2` nor `rumdl` have: an escape hatch to
//! check anything mq's selectors/functions can express, without writing Rust.
//!
//! Unlike the built-in [`crate::RuleId`] (a closed, compile-time enum — that closedness is what
//! keeps built-in rule ids and output stable across releases), a custom rule's id is an
//! arbitrary user-supplied string, so [`CustomDiagnostic`] is a separate, structurally similar
//! type rather than a variant grafted onto the built-in machinery.
//!
//! ```rust
//! use mq_content_lint::custom_rules::{self, CustomRule};
//! use mq_content_lint::Severity;
//!
//! let doc: mq_markdown::Markdown = "![](missing-alt.png)\n".parse().unwrap();
//! let rules = vec![CustomRule {
//!     id: "no_todo_in_alt".to_string(),
//!     query: r#"select(.image.alt == "")"#.to_string(),
//!     message: "image alt text is empty".to_string(),
//!     severity: Severity::Warning,
//! }];
//!
//! let diagnostics = custom_rules::run(&rules, &doc).unwrap();
//! assert_eq!(diagnostics.len(), 1);
//! ```

use serde::Deserialize;

use crate::{Range, Severity};

fn default_severity() -> Severity {
    Severity::Warning
}

/// One `[[custom_rules]]` entry from `mq-content-lint.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRule {
    /// A stable identifier, e.g. `no_todo_comments`. Shown as the diagnostic's rule id; not
    /// checked for collisions against built-in rule ids or other custom rules.
    pub id: String,
    /// An mq query run against the document. Every result that's a markdown node with a source
    /// position becomes one diagnostic; results with no position (e.g. a plain string or number
    /// a query computed) are silently dropped, since there'd be nowhere to point the diagnostic.
    /// See <https://mqlang.org> for query syntax.
    pub query: String,
    /// The diagnostic text shown for every match. The same message is used for every node the
    /// query selects — there's no per-match templating.
    pub message: String,
    #[serde(default = "default_severity")]
    pub severity: Severity,
}

/// A finding from a [`CustomRule`].
#[derive(Debug, Clone, PartialEq)]
pub struct CustomDiagnostic {
    pub rule_id: String,
    pub message: String,
    pub severity: Severity,
    pub range: Option<Range>,
}

/// Error evaluating a custom rule's query — most commonly a syntax mistake in hand-written mq.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("custom rule `{rule_id}`: {reason}")]
pub struct CustomRuleError {
    pub rule_id: String,
    pub reason: String,
}

/// Runs every rule in `rules` against `doc`, returning diagnostics sorted by source position.
///
/// A rule whose query fails to parse or evaluate is reported as an error rather than silently
/// skipped: a typo in a hand-written mq query is a config mistake the user needs to see, not a
/// rule that quietly never fires.
pub fn run(rules: &[CustomRule], doc: &mq_markdown::Markdown) -> Result<Vec<CustomDiagnostic>, CustomRuleError> {
    let mut diagnostics = Vec::new();

    for rule in rules {
        let mut engine = mq_lang::DefaultEngine::default();
        engine.load_builtin_module();

        let nodes: Vec<mq_lang::RuntimeValue> = doc.nodes.iter().cloned().map(mq_lang::RuntimeValue::from).collect();

        let result = engine
            .eval(&rule.query, nodes.into_iter())
            .map_err(|e| CustomRuleError {
                rule_id: rule.id.clone(),
                reason: e.to_string(),
            })?;

        for value in result.compact() {
            if let mq_lang::RuntimeValue::Markdown(node, _) = value
                && let Some(position) = node.position()
            {
                diagnostics.push(CustomDiagnostic {
                    rule_id: rule.id.clone(),
                    message: rule.message.clone(),
                    severity: rule.severity,
                    range: Some(position.into()),
                });
            }
        }
    }

    diagnostics.sort_by_key(|d| d.range.map(|r| (r.start_line, r.start_column)));
    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(query: &str) -> CustomRule {
        CustomRule {
            id: "test_rule".to_string(),
            query: query.to_string(),
            message: "matched".to_string(),
            severity: Severity::Warning,
        }
    }

    #[test]
    fn no_diagnostics_when_query_matches_nothing() {
        let doc: mq_markdown::Markdown = "# Title\n".parse().unwrap();
        let diagnostics = run(&[rule(".image")], &doc).unwrap();
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn one_diagnostic_per_matched_node() {
        let doc: mq_markdown::Markdown = "# One\n\n## Two\n".parse().unwrap();
        let diagnostics = run(&[rule(".h")], &doc).unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].rule_id, "test_rule");
        assert_eq!(diagnostics[0].message, "matched");
        assert_eq!(diagnostics[0].range.unwrap().start_line, 1);
        assert_eq!(diagnostics[1].range.unwrap().start_line, 3);
    }

    #[test]
    fn supports_select_expressions() {
        let doc: mq_markdown::Markdown = "![](missing.png)\n\n![alt text](present.png)\n".parse().unwrap();
        let diagnostics = run(&[rule(r#"select(.image.alt == "")"#)], &doc).unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].range.unwrap().start_line, 1);
    }

    #[test]
    fn severity_is_configurable() {
        let doc: mq_markdown::Markdown = "# Title\n".parse().unwrap();
        let mut r = rule(".h");
        r.severity = Severity::Error;
        let diagnostics = run(&[r], &doc).unwrap();
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn invalid_query_is_a_reported_error_not_a_silent_no_op() {
        let doc: mq_markdown::Markdown = "# Title\n".parse().unwrap();
        let result = run(&[rule("this is not valid mq (((")], &doc);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().rule_id, "test_rule");
    }

    #[test]
    fn multiple_rules_are_all_run_and_merged_in_position_order() {
        let doc: mq_markdown::Markdown = "# Title\n\n![](missing.png)\n".parse().unwrap();
        let rules = vec![
            CustomRule {
                id: "b_rule".to_string(),
                ..rule(".image")
            },
            CustomRule {
                id: "a_rule".to_string(),
                ..rule(".h")
            },
        ];
        let diagnostics = run(&rules, &doc).unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].rule_id, "a_rule");
        assert_eq!(diagnostics[1].rule_id, "b_rule");
    }
}
