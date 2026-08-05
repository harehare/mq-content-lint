//! Rule identity and diagnostic message types.
//!
//! Every lint rule is identified by a [`RuleId`] variant and, when it fires, produces a
//! [`LintMessage`] carrying whatever data is needed to render the diagnostic text. Keeping
//! both as enums (rather than free-form strings) means the compiler enforces that every rule
//! has exactly one ID and that every message variant maps to a real rule.

use std::fmt;
use std::str::FromStr;

/// Unique identifier for a built-in lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleId {
    HeadingHierarchySkip,
    ImageMissingAlt,
    MissingFrontMatterKey,
}

impl RuleId {
    /// All known rule IDs, in a stable order.
    pub const ALL: &'static [RuleId] = &[
        RuleId::HeadingHierarchySkip,
        RuleId::ImageMissingAlt,
        RuleId::MissingFrontMatterKey,
    ];

    /// The rule's `snake_case` identifier, as used in config keys and CLI flags.
    ///
    /// This intentionally matches the corresponding mq selector's name where one exists
    /// (`.h` for headings, `.image`/`.image_ref` for images) so the rule reads as "the mq
    /// selector this rule inspects", not an arbitrary label.
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleId::HeadingHierarchySkip => "heading_hierarchy_skip",
            RuleId::ImageMissingAlt => "image_missing_alt",
            RuleId::MissingFrontMatterKey => "missing_front_matter_key",
        }
    }

    /// The mq selector that a rule inspects, e.g. `.h` for [`RuleId::HeadingHierarchySkip`].
    pub fn selector(&self) -> mq_lang::Selector {
        match self {
            RuleId::HeadingHierarchySkip => mq_lang::Selector::Heading(None),
            RuleId::ImageMissingAlt => mq_lang::Selector::Image,
            RuleId::MissingFrontMatterKey => mq_lang::Selector::Yaml,
        }
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RuleId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RuleId::ALL
            .iter()
            .copied()
            .find(|id| id.as_str() == s)
            .ok_or_else(|| format!("unknown rule id `{s}`"))
    }
}

/// A diagnostic finding, carrying whatever data its rule needs to render a message.
/// Each variant corresponds to exactly one [`RuleId`].
#[derive(Debug, Clone, PartialEq)]
pub enum LintMessage {
    /// A heading's depth jumps by more than one level from the previous heading
    /// (e.g. `#` directly followed by `###`).
    HeadingHierarchySkip { from: u8, to: u8 },
    /// An image (or image reference) has empty alt text.
    ImageMissingAlt { url: String },
    /// The document's front matter is missing a key required by configuration.
    MissingFrontMatterKey { key: String, front_matter_present: bool },
    /// The document's front matter block could not be parsed.
    InvalidFrontMatter { reason: String },
}

impl LintMessage {
    /// The rule that produces this message.
    pub fn rule_id(&self) -> RuleId {
        match self {
            LintMessage::HeadingHierarchySkip { .. } => RuleId::HeadingHierarchySkip,
            LintMessage::ImageMissingAlt { .. } => RuleId::ImageMissingAlt,
            LintMessage::MissingFrontMatterKey { .. } | LintMessage::InvalidFrontMatter { .. } => {
                RuleId::MissingFrontMatterKey
            }
        }
    }

    /// Suggested fix text, if a human reviewer needs a nudge. None of the built-in rules can
    /// be applied automatically (see each rule's `not_autofixable` fixture), so this is
    /// guidance for a human edit rather than a machine-applicable rewrite.
    pub fn help(&self) -> Option<String> {
        match self {
            LintMessage::HeadingHierarchySkip { from, to: _ } => Some(format!(
                "insert an intermediate h{} (or renumber this heading to h{})",
                from + 1,
                from + 1
            )),
            LintMessage::ImageMissingAlt { .. } => {
                Some("describe the image's content or purpose in the alt text".to_string())
            }
            LintMessage::MissingFrontMatterKey {
                key,
                front_matter_present,
            } => Some(if *front_matter_present {
                format!("add `{key}: ...` to the front matter")
            } else {
                format!("add a front matter block with `{key}: ...`")
            }),
            LintMessage::InvalidFrontMatter { .. } => {
                Some("fix the front matter syntax so required keys can be checked".to_string())
            }
        }
    }
}

impl fmt::Display for LintMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LintMessage::HeadingHierarchySkip { from, to } => {
                write!(f, "heading level jumps from h{from} to h{to}, skipping a level")
            }
            LintMessage::ImageMissingAlt { url } => {
                write!(f, "image `{url}` has no alt text")
            }
            LintMessage::MissingFrontMatterKey {
                key,
                front_matter_present,
            } => {
                if *front_matter_present {
                    write!(f, "front matter is missing required key `{key}`")
                } else {
                    write!(
                        f,
                        "document has no front matter block (required key `{key}` is missing)"
                    )
                }
            }
            LintMessage::InvalidFrontMatter { reason } => {
                write!(f, "front matter could not be parsed: {reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_id_round_trips_through_str() {
        for id in RuleId::ALL {
            assert_eq!(id.as_str().parse::<RuleId>().unwrap(), *id);
        }
    }

    #[test]
    fn rule_id_from_str_rejects_unknown() {
        assert!("not_a_real_rule".parse::<RuleId>().is_err());
    }

    #[test]
    fn message_rule_id_matches_intent() {
        let msg = LintMessage::HeadingHierarchySkip { from: 1, to: 3 };
        assert_eq!(msg.rule_id(), RuleId::HeadingHierarchySkip);
        assert_eq!(msg.to_string(), "heading level jumps from h1 to h3, skipping a level");
        assert!(msg.help().unwrap().contains("h2"));
    }
}
