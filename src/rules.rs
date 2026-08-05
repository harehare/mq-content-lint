//! Built-in lint rules.
//!
//! Each rule inspects the `mq-markdown` AST directly rather than running a user-supplied mq
//! query — accepting arbitrary mq expressions as rules is a later stage of this project (see
//! the crate-level docs). [`RuleId::selector`](crate::RuleId::selector) still names the mq
//! selector each rule conceptually corresponds to (`.h`, `.image`, `.yaml`), so a rule reads as
//! "the built-in check for this selector," not an unrelated bespoke concept.

mod heading_hierarchy_skip;
mod image_missing_alt;
mod missing_front_matter_key;

use crate::{Diagnostic, LintConfig, RuleId, Severity};

/// A single built-in lint rule.
pub trait Rule: Send + Sync {
    /// Unique identifier for this rule.
    fn id(&self) -> RuleId;

    /// Default severity when the rule fires and no config override applies.
    fn default_severity(&self) -> Severity;

    /// Analyzes the parsed document and returns any diagnostics.
    ///
    /// Implementations set [`Diagnostic::severity`] to `self.default_severity()`; [`Linter`](crate::Linter)
    /// applies any configured override afterwards, so rules don't need to consult `config` for
    /// severity themselves.
    fn check(&self, doc: &mq_markdown::Markdown, config: &LintConfig) -> Vec<Diagnostic>;
}

/// Returns the full built-in rule set, in a stable order matching [`RuleId::ALL`].
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(heading_hierarchy_skip::HeadingHierarchySkip),
        Box::new(image_missing_alt::ImageMissingAlt),
        Box::new(missing_front_matter_key::MissingFrontMatterKey),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_matches_rule_id_all() {
        let rules = all_rules();
        assert_eq!(rules.len(), RuleId::ALL.len());
        for (rule, id) in rules.iter().zip(RuleId::ALL) {
            assert_eq!(rule.id(), *id);
        }
    }
}
