//! MD044: proper names should use a specific capitalization wherever they appear, configured via
//! `[rules.proper_names] names = ["JavaScript", "GitHub"]`. A no-op with no `names` configured,
//! like [`crate::rules::missing_front_matter_key`]. Scans [`mq_markdown::Node::Text`] content
//! only, so a name that's already correctly cased inside code or a URL is left alone.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct ProperNames;

fn is_word_boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| !c.is_alphanumeric())
}

/// Finds case-mismatched occurrences of `name` in `text` at word boundaries.
fn find_mismatches(text: &str, name: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i + name_chars.len() <= chars.len() {
        let window = &chars[i..i + name_chars.len()];
        let matches_ci = window.iter().zip(&name_chars).all(|(a, b)| a.eq_ignore_ascii_case(b));
        if matches_ci {
            let before = if i == 0 { None } else { Some(chars[i - 1]) };
            let after = chars.get(i + name_chars.len()).copied();
            if is_word_boundary(before) && is_word_boundary(after) {
                if window != name_chars.as_slice() {
                    result.push((i, i + name_chars.len()));
                }
                i += name_chars.len();
                continue;
            }
        }
        i += 1;
    }
    result
}

impl Rule for ProperNames {
    fn id(&self) -> RuleId {
        RuleId::ProperNames
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let names = config
            .rule_options(self.id())
            .get_str_array("names")
            .unwrap_or_default();
        if names.is_empty() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Text(text) = node else { return };
            let Some(position) = &text.position else { return };
            if position.start.line != position.end.line {
                return;
            }
            for name in &names {
                for (start, end) in find_mismatches(&text.value, name) {
                    let found: String = text.value.chars().skip(start).take(end - start).collect();
                    let range = Range::single_line(
                        position.start.line,
                        position.start.column + start,
                        position.start.column + end,
                    );
                    diagnostics.push(
                        Diagnostic::new(
                            LintMessage::ProperNames {
                                found,
                                expected: name.clone(),
                            },
                            self.default_severity(),
                        )
                        .with_range(range)
                        .with_fix(Fix::new(range, name.clone())),
                    );
                }
            }
        });
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["names"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str, names: &[&str]) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        let list = names.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
        let config = LintConfig::from_toml_str(&format!("[rules.proper_names]\nnames = [{list}]\n")).unwrap();
        ProperNames.check(&doc, markdown, &config)
    }

    #[test]
    fn no_op_with_no_config() {
        let doc: mq_markdown::Markdown = "javascript is great\n".parse().unwrap();
        assert!(
            ProperNames
                .check(&doc, "javascript is great\n", &LintConfig::default())
                .is_empty()
        );
    }

    #[test]
    fn no_diagnostics_for_correct_casing() {
        assert!(run("JavaScript is great\n", &["JavaScript"]).is_empty());
    }

    #[test]
    fn flags_and_fixes_incorrect_casing() {
        let diagnostics = run("javascript is great\n", &["JavaScript"]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "JavaScript");
    }
}
