//! Unifies built-in rule diagnostics with [custom rule](mq_content_lint::custom_rules)
//! diagnostics into one shape the output formatters can report without caring which kind they're
//! looking at.
//!
//! This lives in the binary, not the library: `mq_content_lint::Diagnostic`'s rule identity is
//! the closed, compile-time [`mq_content_lint::RuleId`] enum on purpose (see that type's docs) —
//! a custom rule's id is an arbitrary user string, so it can never be a `Diagnostic` and has no
//! business trying to be one. The CLI is the layer that needs to show both kinds side by side.

use mq_content_lint::custom_rules::CustomDiagnostic;
use mq_content_lint::{Diagnostic, Range, Severity};

pub(crate) enum ReportItem {
    Builtin(Diagnostic),
    Custom(CustomDiagnostic),
}

impl ReportItem {
    pub(crate) fn severity(&self) -> Severity {
        match self {
            ReportItem::Builtin(d) => d.severity,
            ReportItem::Custom(d) => d.severity,
        }
    }

    pub(crate) fn range(&self) -> Option<Range> {
        match self {
            ReportItem::Builtin(d) => d.range,
            ReportItem::Custom(d) => d.range,
        }
    }

    /// The rule id string: a built-in's `snake_case` name, or a custom rule's configured `id`.
    pub(crate) fn rule_id(&self) -> &str {
        match self {
            ReportItem::Builtin(d) => d.rule_id().as_str(),
            ReportItem::Custom(d) => &d.rule_id,
        }
    }

    /// The mq selector a built-in rule corresponds to; `None` for rules with no single selector
    /// (several built-ins) and for every custom rule (its query may use several, or none at
    /// all — a custom rule's "selector" is however much of its query the user wrote).
    pub(crate) fn selector(&self) -> Option<mq_lang::Selector> {
        match self {
            ReportItem::Builtin(d) => d.rule_id().selector(),
            ReportItem::Custom(_) => None,
        }
    }

    pub(crate) fn text(&self) -> String {
        match self {
            ReportItem::Builtin(d) => d.text(),
            ReportItem::Custom(d) => d.message.clone(),
        }
    }

    pub(crate) fn help(&self) -> Option<String> {
        match self {
            ReportItem::Builtin(d) => d.help(),
            ReportItem::Custom(_) => None,
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
