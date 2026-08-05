//! Fixture-driven tests for the three built-in rules.
//!
//! Each rule under `tests/fixtures/<rule>/` has three files:
//! - `ok.md`: triggers no diagnostics for that rule.
//! - `bad.md`: triggers the rule, with a fix a human could plausibly apply.
//! - `not_autofixable.md`: triggers the rule in a case where there is no single correct
//!   mechanical fix (ambiguous heading levels, no data to derive alt text from, no way to
//!   invent required front matter content). `mq-content-lint` has no `--fix` flag and
//!   `Diagnostic` carries no machine-applicable rewrite at all (see `mq_content_lint::Diagnostic`
//!   docs) — every diagnostic is equally "not autofixable" by construction — so this fixture
//!   exists to document, with a concrete example, *why* that's the right design for this rule
//!   rather than a gap to fill in later.

use std::path::Path;

use mq_content_lint::{LintConfig, Linter, RuleId};

fn fixture(rule_dir: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rule_dir)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading fixture {}: {e}", path.display()))
}

fn diagnostics_for(rule_id: RuleId, markdown: &str, config: &LintConfig) -> Vec<mq_content_lint::Diagnostic> {
    let doc: mq_markdown::Markdown = markdown.parse().expect("fixture must be valid markdown");
    Linter::with_default_rules()
        .run(&doc, config)
        .into_iter()
        .filter(|d| d.rule_id() == rule_id)
        .collect()
}

mod heading_hierarchy_skip {
    use super::*;

    const RULE: RuleId = RuleId::HeadingHierarchySkip;
    const DIR: &str = "heading_hierarchy_skip";

    #[test]
    fn ok_fixture_has_no_diagnostics() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "ok.md"), &LintConfig::default());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn bad_fixture_flags_the_skip() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "bad.md"), &LintConfig::default());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].text().contains("h1 to h3"));
        assert_eq!(diagnostics[0].range.unwrap().start_line, 3);
    }

    #[test]
    fn not_autofixable_fixture_flags_the_skip() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "not_autofixable.md"), &LintConfig::default());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].text().contains("h2 to h5"));
    }
}

mod image_missing_alt {
    use super::*;

    const RULE: RuleId = RuleId::ImageMissingAlt;
    const DIR: &str = "image_missing_alt";

    #[test]
    fn ok_fixture_has_no_diagnostics() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "ok.md"), &LintConfig::default());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn bad_fixture_flags_the_empty_alt() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "bad.md"), &LintConfig::default());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].text().contains("missing-alt.png"));
        assert_eq!(diagnostics[0].range.unwrap().start_line, 3);
    }

    #[test]
    fn not_autofixable_fixture_flags_the_data_uri_image() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "not_autofixable.md"), &LintConfig::default());
        assert_eq!(diagnostics.len(), 1);
    }
}

mod missing_front_matter_key {
    use super::*;

    const RULE: RuleId = RuleId::MissingFrontMatterKey;
    const DIR: &str = "missing_front_matter_key";

    fn config() -> LintConfig {
        LintConfig::from_toml_str("[front_matter]\nrequired_keys = [\"title\", \"date\"]\n").unwrap()
    }

    #[test]
    fn ok_fixture_has_no_diagnostics() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "ok.md"), &config());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn bad_fixture_flags_the_missing_key() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "bad.md"), &config());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].text().contains('`'));
        assert!(diagnostics[0].text().contains("date"));
    }

    #[test]
    fn not_autofixable_fixture_flags_every_required_key() {
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "not_autofixable.md"), &config());
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].text().contains("no front matter block"));
    }

    #[test]
    fn rule_is_a_no_op_without_config() {
        // Same fixture, default config: no required keys means no diagnostics at all — the
        // deterministic "no config" behavior documented on `LintConfig`.
        let diagnostics = diagnostics_for(RULE, &fixture(DIR, "not_autofixable.md"), &LintConfig::default());
        assert!(diagnostics.is_empty());
    }
}

/// Every diagnostic in this crate is reported "as is" — there is no `Fix`/rewrite type and no
/// CLI `--fix` flag anywhere in the public API, so every finding across all three fixture sets
/// is, by construction, one a human must resolve.
#[test]
fn no_diagnostic_type_exposes_a_machine_applicable_fix() {
    let doc: mq_markdown::Markdown = fixture("heading_hierarchy_skip", "bad.md").parse().unwrap();
    let diagnostics = Linter::with_default_rules().run(&doc, &LintConfig::default());
    assert!(!diagnostics.is_empty());
    // `Diagnostic` has exactly three fields: `message`, `severity`, `range`. If a `fix` field
    // is ever added, this destructure forces this test (and its surrounding claim) to be
    // updated deliberately rather than silently going stale.
    let mq_content_lint::Diagnostic {
        message: _,
        severity: _,
        range: _,
    } = diagnostics.into_iter().next().unwrap();
}

/// Rule ids and mq selectors are part of the stable, documented output surface (config keys,
/// JSON/SARIF `ruleId`), so pin their exact strings here.
#[test]
fn rule_ids_and_selectors_are_stable() {
    assert_eq!(RuleId::HeadingHierarchySkip.as_str(), "heading_hierarchy_skip");
    assert_eq!(RuleId::HeadingHierarchySkip.selector().to_string(), ".h");
    assert_eq!(RuleId::ImageMissingAlt.as_str(), "image_missing_alt");
    assert_eq!(RuleId::ImageMissingAlt.selector().to_string(), ".image");
    assert_eq!(RuleId::MissingFrontMatterKey.as_str(), "missing_front_matter_key");
    assert_eq!(RuleId::MissingFrontMatterKey.selector().to_string(), ".yaml");
}

/// With no config file, linting the same document twice must produce byte-identical
/// diagnostics (same rule ids, same order, same positions) — the "decisiona are deterministic
/// with no config" acceptance bar.
#[test]
fn default_rules_run_deterministically_with_no_config() {
    let markdown = format!(
        "{}\n{}\n{}",
        fixture("heading_hierarchy_skip", "bad.md"),
        fixture("image_missing_alt", "bad.md"),
        fixture("missing_front_matter_key", "bad.md"),
    );
    let doc: mq_markdown::Markdown = markdown.parse().unwrap();
    let config = LintConfig::default();
    let linter = Linter::with_default_rules();

    let first = linter.run(&doc, &config);
    let second = linter.run(&doc, &config);
    assert_eq!(first, second);
    assert!(!first.is_empty());
}
