//! Checks that the document's front matter (YAML `---` or TOML `+++` block) defines every key
//! listed in `front_matter.required_keys` (see [`crate::config`]).
//!
//! With no `required_keys` configured, this rule never fires — there's no universally sensible
//! default key to require, so "front matter has no configured requirement" is the deterministic
//! no-config behavior, not "silently pick one."

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct MissingFrontMatterKey;

/// The document's front matter block, if any, decoded into its top-level keys.
enum FrontMatter {
    /// No `---`/`+++` block was found at all.
    Absent,
    /// A block was found but couldn't be parsed as YAML/TOML.
    Invalid {
        position: Option<mq_markdown::Position>,
        reason: String,
    },
    /// Successfully parsed; `keys` are its top-level mapping keys (empty if the block isn't a
    /// mapping at all, e.g. a bare scalar or list).
    Present {
        position: Option<mq_markdown::Position>,
        keys: Vec<String>,
    },
}

fn find_front_matter(doc: &mq_markdown::Markdown) -> FrontMatter {
    for node in &doc.nodes {
        match node {
            Node::Yaml(yaml) => {
                return match serde_yaml::from_str::<serde_yaml::Value>(&yaml.value) {
                    Ok(serde_yaml::Value::Mapping(mapping)) => FrontMatter::Present {
                        position: yaml.position.clone(),
                        keys: mapping.keys().filter_map(|k| k.as_str().map(str::to_string)).collect(),
                    },
                    Ok(_) => FrontMatter::Present {
                        position: yaml.position.clone(),
                        keys: Vec::new(),
                    },
                    Err(e) => FrontMatter::Invalid {
                        position: yaml.position.clone(),
                        reason: e.to_string(),
                    },
                };
            }
            Node::Toml(toml_node) => {
                return match toml::from_str::<toml::Value>(&toml_node.value) {
                    Ok(toml::Value::Table(table)) => FrontMatter::Present {
                        position: toml_node.position.clone(),
                        keys: table.keys().cloned().collect(),
                    },
                    Ok(_) => FrontMatter::Present {
                        position: toml_node.position.clone(),
                        keys: Vec::new(),
                    },
                    Err(e) => FrontMatter::Invalid {
                        position: toml_node.position.clone(),
                        reason: e.to_string(),
                    },
                };
            }
            _ => {}
        }
    }
    FrontMatter::Absent
}

impl Rule for MissingFrontMatterKey {
    fn id(&self) -> RuleId {
        RuleId::MissingFrontMatterKey
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        if config.required_front_matter_keys.is_empty() {
            return Vec::new();
        }

        let diagnostic_at = |message: LintMessage, position: &Option<mq_markdown::Position>| {
            let mut diagnostic = Diagnostic::new(message, self.default_severity());
            if let Some(position) = position.clone() {
                diagnostic = diagnostic.with_range(position);
            }
            diagnostic
        };

        match find_front_matter(doc) {
            FrontMatter::Absent => config
                .required_front_matter_keys
                .iter()
                .map(|key| {
                    diagnostic_at(
                        LintMessage::MissingFrontMatterKey {
                            key: key.clone(),
                            front_matter_present: false,
                        },
                        &None,
                    )
                })
                .collect(),
            FrontMatter::Invalid { position, reason } => {
                vec![diagnostic_at(LintMessage::InvalidFrontMatter { reason }, &position)]
            }
            FrontMatter::Present { position, keys } => config
                .required_front_matter_keys
                .iter()
                .filter(|key| !keys.contains(key))
                .map(|key| {
                    diagnostic_at(
                        LintMessage::MissingFrontMatterKey {
                            key: key.clone(),
                            front_matter_present: true,
                        },
                        &position,
                    )
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str, required_keys: &[&str]) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        let config = LintConfig::from_toml_str(&format!(
            "[front_matter]\nrequired_keys = [{}]\n",
            required_keys
                .iter()
                .map(|k| format!("{k:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .unwrap();
        MissingFrontMatterKey.check(&doc, markdown, &config)
    }

    #[test]
    fn no_diagnostics_with_no_required_keys_configured() {
        assert!(run("# Title\n", &[]).is_empty());
        assert!(run("---\ntitle: Hello\n---\n\n# Title\n", &[]).is_empty());
    }

    #[test]
    fn no_diagnostics_when_all_required_keys_are_present() {
        assert!(
            run(
                "---\ntitle: Hello\ndate: 2024-01-01\n---\n\n# Title\n",
                &["title", "date"]
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_a_missing_key_when_front_matter_exists() {
        let diagnostics = run("---\ntitle: Hello\n---\n\n# Title\n", &["title", "date"]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::MissingFrontMatterKey {
                key: "date".to_string(),
                front_matter_present: true,
            }
        );
    }

    #[test]
    fn flags_every_required_key_when_front_matter_is_entirely_absent() {
        let diagnostics = run("# Title\n\nNo front matter here.\n", &["title", "date"]);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|d| matches!(
            &d.message,
            LintMessage::MissingFrontMatterKey {
                front_matter_present: false,
                ..
            }
        )));
    }

    #[test]
    fn flags_invalid_yaml_front_matter() {
        let diagnostics = run("---\ntitle: [unterminated\n---\n\n# Title\n", &["title"]);
        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(diagnostics[0].message, LintMessage::InvalidFrontMatter { .. }));
    }

    #[test]
    fn supports_toml_front_matter() {
        assert!(run("+++\ntitle = \"Hello\"\n+++\n\n# Title\n", &["title"]).is_empty());
        assert_eq!(
            run("+++\ntitle = \"Hello\"\n+++\n\n# Title\n", &["title", "date"]).len(),
            1
        );
    }
}
