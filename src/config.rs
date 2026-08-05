//! Configuration for `mq-content-lint`, loaded from a `mq-content-lint.toml` file.
//!
//! With no config file at all, every built-in rule runs at its default severity except
//! [`crate::RuleId::MissingFrontMatterKey`], which is a no-op until `front_matter.required_keys`
//! names at least one key — there is no sensible key to require by default, so the behavior
//! with no config is simply "don't check front matter keys", not "guess some".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{RuleId, Severity};

/// The config file name `mq-content-lint` looks for, both when given explicitly via `--config`
/// and when auto-discovered by walking up from the linted file's directory.
pub const CONFIG_FILE_NAME: &str = "mq-content-lint.toml";

/// Per-rule setting: either a plain enable/disable flag, or a severity override (which also
/// implies the rule is enabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum RuleSetting {
    Enabled(bool),
    Severity(Severity),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct RulesTable {
    heading_hierarchy_skip: Option<RuleSetting>,
    image_missing_alt: Option<RuleSetting>,
    missing_front_matter_key: Option<RuleSetting>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FrontMatterTable {
    required_keys: Vec<String>,
}

/// The raw shape of `mq-content-lint.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    rules: RulesTable,
    front_matter: FrontMatterTable,
}

/// Error loading or parsing a config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
}

/// Resolved linter configuration.
///
/// Construct via [`LintConfig::from_toml_str`], [`LintConfig::load_from_path`], or
/// [`LintConfig::discover`]; use [`LintConfig::default`] for the deterministic no-config
/// behavior described in the module docs.
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    rules: HashMap<RuleId, RuleSetting>,
    pub required_front_matter_keys: Vec<String>,
}

impl LintConfig {
    /// Parses a `mq-content-lint.toml` document from a string.
    pub fn from_toml_str(toml_str: &str) -> Result<Self, Box<toml::de::Error>> {
        let file: FileConfig = toml::from_str(toml_str).map_err(Box::new)?;
        Ok(Self::from_file_config(file))
    }

    fn from_file_config(file: FileConfig) -> Self {
        let mut rules = HashMap::new();
        if let Some(setting) = file.rules.heading_hierarchy_skip {
            rules.insert(RuleId::HeadingHierarchySkip, setting);
        }
        if let Some(setting) = file.rules.image_missing_alt {
            rules.insert(RuleId::ImageMissingAlt, setting);
        }
        if let Some(setting) = file.rules.missing_front_matter_key {
            rules.insert(RuleId::MissingFrontMatterKey, setting);
        }

        Self {
            rules,
            required_front_matter_keys: file.front_matter.required_keys,
        }
    }

    /// Loads and parses a config file at an explicit path.
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&content).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Walks up from `start_dir` looking for [`CONFIG_FILE_NAME`], loading the first one found.
    /// Returns the default config (equivalent to no config file) if none is found before
    /// reaching the filesystem root.
    pub fn discover(start_dir: &Path) -> Result<Self, ConfigError> {
        let mut dir = Some(start_dir);
        while let Some(d) = dir {
            let candidate = d.join(CONFIG_FILE_NAME);
            if candidate.is_file() {
                return Self::load_from_path(&candidate);
            }
            dir = d.parent();
        }
        Ok(Self::default())
    }

    /// Disables a rule, overriding whatever the config file said. Used by the CLI's `--disable`
    /// flag, which always wins over the config file.
    pub fn disable_rule(&mut self, rule_id: RuleId) {
        self.rules.insert(rule_id, RuleSetting::Enabled(false));
    }

    /// Returns `true` if the given rule should run.
    pub fn is_rule_enabled(&self, rule_id: RuleId) -> bool {
        match self.rules.get(&rule_id) {
            Some(RuleSetting::Enabled(enabled)) => *enabled,
            Some(RuleSetting::Severity(_)) | None => true,
        }
    }

    /// Returns the effective severity for a rule: a configured override if present, otherwise
    /// the rule's own default.
    pub fn severity_for(&self, rule_id: RuleId, default: Severity) -> Severity {
        match self.rules.get(&rule_id) {
            Some(RuleSetting::Severity(severity)) => *severity,
            _ => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_all_rules_at_default_severity() {
        let config = LintConfig::default();
        for id in RuleId::ALL {
            assert!(config.is_rule_enabled(*id));
        }
        assert!(config.required_front_matter_keys.is_empty());
    }

    #[test]
    fn rule_can_be_disabled_with_a_bool() {
        let config = LintConfig::from_toml_str(
            r#"
            [rules]
            image_missing_alt = false
            "#,
        )
        .unwrap();
        assert!(!config.is_rule_enabled(RuleId::ImageMissingAlt));
        assert!(config.is_rule_enabled(RuleId::HeadingHierarchySkip));
    }

    #[test]
    fn rule_severity_can_be_overridden_with_a_string() {
        let config = LintConfig::from_toml_str(
            r#"
            [rules]
            heading_hierarchy_skip = "error"
            "#,
        )
        .unwrap();
        assert!(config.is_rule_enabled(RuleId::HeadingHierarchySkip));
        assert_eq!(
            config.severity_for(RuleId::HeadingHierarchySkip, Severity::Warning),
            Severity::Error
        );
    }

    #[test]
    fn front_matter_required_keys_are_parsed() {
        let config = LintConfig::from_toml_str(
            r#"
            [front_matter]
            required_keys = ["title", "date"]
            "#,
        )
        .unwrap();
        assert_eq!(config.required_front_matter_keys, vec!["title", "date"]);
    }

    #[test]
    fn unknown_rule_name_is_a_parse_error() {
        let result = LintConfig::from_toml_str(
            r#"
            [rules]
            not_a_real_rule = true
            "#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn invalid_severity_string_is_a_parse_error() {
        let result = LintConfig::from_toml_str(
            r#"
            [rules]
            image_missing_alt = "critical"
            "#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn discover_finds_config_in_an_ancestor_directory() {
        let dir = tempdir();
        std::fs::write(
            dir.join(CONFIG_FILE_NAME),
            "[front_matter]\nrequired_keys = [\"title\"]\n",
        )
        .unwrap();
        let nested = dir.join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();

        let config = LintConfig::discover(&nested).unwrap();
        assert_eq!(config.required_front_matter_keys, vec!["title"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discover_returns_default_when_nothing_found() {
        let dir = tempdir();
        let config = LintConfig::discover(&dir).unwrap();
        assert!(config.required_front_matter_keys.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-test-{}", uid()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uid() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
    }
}
