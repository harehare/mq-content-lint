//! Static content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s
//! `mq-markdown` AST.
//!
//! `mq-content-lint` inspects the *content* of a Markdown document — heading structure, list and
//! table consistency, whitespace, link/image hygiene, required front matter — comprehensive
//! coverage of [markdownlint](https://github.com/DavidAnson/markdownlint)'s rule set, expressed
//! against mq's own node model instead of a bespoke rule engine. It is a separate tool from
//! `mq-lint` (which lints `.mq` query scripts, not Markdown content). Beyond the 53 built-in
//! rules, [`custom_rules`] lets a config file define its own rules as mq queries — the one
//! capability neither markdownlint nor rumdl offer.
//!
//! ## Example
//!
//! ```rust
//! use mq_content_lint::{LintConfig, Linter};
//!
//! let source = "# Title\n\n### Skipped a level\n";
//! let doc: mq_markdown::Markdown = source.parse().unwrap();
//! let config = LintConfig::default();
//! let linter = Linter::with_default_rules();
//! let diagnostics = linter.run(&doc, source, &config);
//!
//! assert_eq!(diagnostics.len(), 1);
//! ```

pub mod config;
pub mod custom_rules;
pub mod fix;
pub mod message;
pub mod report_item;
pub mod rules;
mod text;
mod walk;

pub use config::LintConfig;
pub use fix::Fix;
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

/// A source location for a diagnostic or fix, 1-based like `mq_markdown::Position`, but `Copy`
/// and serializable independent of the `json` feature on `mq-markdown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Range {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl Range {
    /// A range confined to a single line, from `start_column` to `end_column`.
    pub fn single_line(line: usize, start_column: usize, end_column: usize) -> Self {
        Self {
            start_line: line,
            start_column,
            end_line: line,
            end_column,
        }
    }

    /// A zero-width range at `line`, `column` — an insertion point rather than a span to
    /// replace, for fixes that only ever add text (e.g. inserting a blank line).
    pub fn at(line: usize, column: usize) -> Self {
        Self::single_line(line, column, column)
    }
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
/// `fix` is `None` for rules where there's no single correct mechanical rewrite (e.g. no
/// reasonable default alt text, no way to invent required front matter content). Rules that can
/// be applied automatically populate it; `mq-content-lint --fix` applies every diagnostic's fix
/// in one pass over the original source (diagnostics are not recomputed between fixes, matching
/// `mq-lint`'s `--fix` behavior).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub message: LintMessage,
    pub severity: Severity,
    /// Source location of the finding, if the offending node carried position info.
    pub range: Option<Range>,
    pub fix: Option<Fix>,
}

impl Diagnostic {
    pub fn new(message: LintMessage, severity: Severity) -> Self {
        Self {
            message,
            severity,
            range: None,
            fix: None,
        }
    }

    pub fn with_range(mut self, range: impl Into<Range>) -> Self {
        self.range = Some(range.into());
        self
    }

    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
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
    ///
    /// `source` is the raw text `doc` was parsed from — some rules (whitespace, line length,
    /// exact heading/list marker syntax) need it because the AST alone doesn't preserve it.
    pub fn run(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics: Vec<Diagnostic> = self
            .rule_set
            .iter()
            .filter(|rule| config.is_rule_enabled(rule.id()))
            .flat_map(|rule| {
                rule.check(doc, source, config).into_iter().map(|mut d| {
                    d.severity = config.severity_for(rule.id(), rule.default_severity());
                    d
                })
            })
            .collect();

        diagnostics.sort_by_key(|d| d.range.map(|r| (r.start_line, r.start_column)));
        diagnostics
    }
}
