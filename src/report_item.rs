//! Unifies built-in rule diagnostics with [custom rule](crate::custom_rules) diagnostics into one
//! shape callers can report without caring which kind they're looking at.
//!
//! `crate::Diagnostic`'s rule identity is the closed, compile-time [`crate::RuleId`] enum on
//! purpose (see that type's docs) — a custom rule's id is an arbitrary user string, so it can
//! never be a `Diagnostic` and has no business trying to be one. Both the CLI and the LSP server
//! need to show both kinds side by side, which is what this type (and [`lint`], which produces
//! it) is for.

use crate::custom_rules::{CustomDiagnostic, CustomRuleError};
use crate::{Diagnostic, Fix, LintConfig, Linter, Range, Severity};

pub enum ReportItem {
    Builtin(Diagnostic),
    Custom(CustomDiagnostic),
}

impl ReportItem {
    pub fn severity(&self) -> Severity {
        match self {
            ReportItem::Builtin(d) => d.severity,
            ReportItem::Custom(d) => d.severity,
        }
    }

    pub fn range(&self) -> Option<Range> {
        match self {
            ReportItem::Builtin(d) => d.range,
            ReportItem::Custom(d) => d.range,
        }
    }

    /// The rule id string: a built-in's `snake_case` name, or a custom rule's configured `id`.
    pub fn rule_id(&self) -> &str {
        match self {
            ReportItem::Builtin(d) => d.rule_id().as_str(),
            ReportItem::Custom(d) => &d.rule_id,
        }
    }

    /// The mq selector a built-in rule corresponds to; `None` for rules with no single selector
    /// (several built-ins) and for every custom rule (its query may use several, or none at
    /// all — a custom rule's "selector" is however much of its query the user wrote).
    pub fn selector(&self) -> Option<mq_lang::Selector> {
        match self {
            ReportItem::Builtin(d) => d.rule_id().selector(),
            ReportItem::Custom(_) => None,
        }
    }

    pub fn text(&self) -> String {
        match self {
            ReportItem::Builtin(d) => d.text(),
            ReportItem::Custom(d) => d.message.clone(),
        }
    }

    pub fn help(&self) -> Option<String> {
        match self {
            ReportItem::Builtin(d) => d.help(),
            ReportItem::Custom(_) => None,
        }
    }

    /// A machine-applicable rewrite, if this diagnostic has one — a built-in rule's own fix, or
    /// a custom rule's configured `fix` expression's result.
    pub fn fix(&self) -> Option<&Fix> {
        match self {
            ReportItem::Builtin(d) => d.fix.as_ref(),
            ReportItem::Custom(d) => d.fix.as_ref(),
        }
    }
}

impl From<Diagnostic> for ReportItem {
    fn from(d: Diagnostic) -> Self {
        ReportItem::Builtin(d)
    }
}

impl From<CustomDiagnostic> for ReportItem {
    fn from(d: CustomDiagnostic) -> Self {
        ReportItem::Custom(d)
    }
}

/// Runs both built-in and custom rules against `doc`/`source`, merging their diagnostics into a
/// single position-sorted list. The shared entry point behind the CLI, `--fix`, and the LSP
/// server — none of them should reimplement "run the built-ins, run the custom rules, merge, and
/// sort" on their own.
pub fn lint(
    doc: &mq_markdown::Markdown,
    source: &str,
    linter: &Linter,
    config: &LintConfig,
) -> Result<Vec<ReportItem>, CustomRuleError> {
    let mut items: Vec<ReportItem> = linter
        .run(doc, source, config)
        .into_iter()
        .map(ReportItem::from)
        .collect();

    let custom = crate::custom_rules::run(&config.custom_rules, doc)?;
    items.extend(custom.into_iter().map(ReportItem::from));

    items.sort_by_key(|item| item.range().map(|r| (r.start_line, r.start_column)));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_merges_and_sorts_builtin_and_custom_diagnostics() {
        let source = "# Title\n\n![](missing-alt.png)\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let config = LintConfig::from_toml_str(
            r#"
            [[custom_rules]]
            id = "no_todo"
            query = '.h'
            message = "found a heading"
            "#,
        )
        .unwrap();
        let linter = Linter::with_default_rules();

        let items = lint(&doc, source, &linter, &config).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].rule_id(), "no_todo");
        assert_eq!(items[0].range().unwrap().start_line, 1);
        assert_eq!(items[1].rule_id(), "image_missing_alt");
        assert_eq!(items[1].range().unwrap().start_line, 3);
    }

    #[test]
    fn lint_propagates_a_custom_rule_error() {
        let source = "# Title\n";
        let doc: mq_markdown::Markdown = source.parse().unwrap();
        let config = LintConfig::from_toml_str(
            r#"
            [[custom_rules]]
            id = "broken"
            query = "this is not valid mq((("
            message = "never fires"
            "#,
        )
        .unwrap();
        let linter = Linter::with_default_rules();

        assert!(lint(&doc, source, &linter, &config).is_err());
    }
}
