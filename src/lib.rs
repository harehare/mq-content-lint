//! Static content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s
//! `mq-markdown` AST.
//!
//! `mq-content-lint` inspects the *content* of a Markdown document — heading structure, image
//! accessibility, required front matter — the same territory as markdownlint's structural rules
//! and Vale's shareable styles, but expressed against mq's own node model instead of a bespoke
//! rule engine. It is a separate tool from `mq-lint` (which lints `.mq` query scripts, not
//! Markdown content) and from arbitrary user-supplied mq queries as rules, which is a later
//! stage of this project.
//!
//! ## Example
//!
//! ```rust
//! use mq_content_lint::{LintConfig, Linter};
//!
//! let doc: mq_markdown::Markdown = "# Title\n\n### Skipped a level\n".parse().unwrap();
//! let config = LintConfig::default();
//! let linter = Linter::with_default_rules();
//! let diagnostics = linter.run(&doc, &config);
//!
//! assert_eq!(diagnostics.len(), 1);
//! ```

pub mod config;
pub mod message;
pub mod rules;
mod walk;

pub use config::LintConfig;
pub use message::{LintMessage, RuleId};

use serde::Serialize;

/// Severity level for a lint diagnostic.
///
/// Ordered least to most severe so a `Vec<Diagnostic>` sorts naturally and groups compare with
/// `<`/`>`; SARIF output maps `Error` to `error`, `Warning` to `warning`, and `Info` to `note`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A source location for a diagnostic, mirroring `mq_markdown::Position` but `Copy` and
/// serializable independent of the `json` feature on `mq-markdown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Range {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl From<mq_markdown::Position> for Range {
    fn from(position: mq_markdown::Position) -> Self {
        Self {
            start_line: position.start.line,
            start_column: position.start.column,
            end_line: position.end.line,
            end_column: position.end.column,
        }
    }
}

/// A lint finding produced by a [`rules::Rule`].
///
/// None of the built-in rules can be applied automatically — there is no reasonable default
/// alt text, no single correct place to insert a skipped heading level, and no way to invent
/// the value of a missing front matter key — so unlike `mq-lint`, `Diagnostic` carries no
/// machine-applicable fix. See each rule's `not_autofixable` fixture under `tests/fixtures`.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub message: LintMessage,
    pub severity: Severity,
    /// Source location of the finding, if the offending node carried position info.
    pub range: Option<Range>,
}

impl Diagnostic {
    pub fn new(message: LintMessage, severity: Severity) -> Self {
        Self {
            message,
            severity,
            range: None,
        }
    }

    pub fn with_range(mut self, range: impl Into<Range>) -> Self {
        self.range = Some(range.into());
        self
    }

    /// The rule that produced this diagnostic.
    pub fn rule_id(&self) -> RuleId {
        self.message.rule_id()
    }

    /// Human-readable diagnostic text.
    pub fn text(&self) -> String {
        self.message.to_string()
    }

    /// Suggested action for a human reviewer, if any.
    pub fn help(&self) -> Option<String> {
        self.message.help()
    }
}

/// Runs all registered [`rules::Rule`]s against a parsed document.
#[derive(Default)]
pub struct Linter {
    rule_set: Vec<Box<dyn rules::Rule>>,
}

impl Linter {
    /// Create a linter with the full built-in rule set.
    pub fn with_default_rules() -> Self {
        Self {
            rule_set: rules::all_rules(),
        }
    }

    /// Lints a parsed document, returning diagnostics sorted by source position.
    pub fn run(&self, doc: &mq_markdown::Markdown, config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = self
            .rule_set
            .iter()
            .filter(|rule| config.is_rule_enabled(rule.id()))
            .flat_map(|rule| {
                rule.check(doc, config).into_iter().map(|mut d| {
                    d.severity = config.severity_for(rule.id(), rule.default_severity());
                    d
                })
            })
            .collect();

        diagnostics.sort_by_key(|d| d.range.map(|r| (r.start_line, r.start_column)));
        diagnostics
    }
}
