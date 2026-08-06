//! MD043: the document's headings must match a required structure, configured as
//! `[rules.required_headings] headings = ["# Title", "## Overview", "*", "## Conclusion"]`
//! (`"#"`-prefixed level + text per entry; `"*"` matches any single heading). A no-op with no
//! `headings` configured, like [`crate::rules::missing_front_matter_key`]. Not auto-fixable —
//! restructuring a document's headings is an editorial call.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct RequiredHeadings;

impl Rule for RequiredHeadings {
    fn id(&self) -> RuleId {
        RuleId::RequiredHeadings
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let expected = config
            .rule_options(self.id())
            .get_str_array("headings")
            .unwrap_or_default();
        if expected.is_empty() {
            return Vec::new();
        }

        let mut headings: Vec<(String, Option<mq_markdown::Position>)> = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Heading(heading) = node {
                headings.push((
                    format!("{} {}", "#".repeat(heading.depth as usize), node.value()),
                    heading.position.clone(),
                ));
            }
        });
        headings.sort_by_key(|(_, pos)| pos.as_ref().map(|p| (p.start.line, p.start.column)));
        let found: Vec<String> = headings.iter().map(|(text, _)| text.clone()).collect();

        let matches = found.len() == expected.len() && found.iter().zip(&expected).all(|(f, e)| e == "*" || f == e);

        if matches {
            return Vec::new();
        }

        vec![Diagnostic::new(
            LintMessage::RequiredHeadings {
                expected: expected.join(" > "),
                found: found.join(" > "),
            },
            self.default_severity(),
        )]
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["headings"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str, headings: &[&str]) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        let list = headings.iter().map(|h| format!("{h:?}")).collect::<Vec<_>>().join(", ");
        let config = LintConfig::from_toml_str(&format!("[rules.required_headings]\nheadings = [{list}]\n")).unwrap();
        RequiredHeadings.check(&doc, markdown, &config)
    }

    #[test]
    fn no_op_with_no_config() {
        let doc: mq_markdown::Markdown = "# Anything\n".parse().unwrap();
        assert!(
            RequiredHeadings
                .check(&doc, "# Anything\n", &LintConfig::default())
                .is_empty()
        );
    }

    #[test]
    fn no_diagnostics_for_a_matching_structure() {
        assert!(run("# Title\n\n## Overview\n", &["# Title", "## Overview"]).is_empty());
    }

    #[test]
    fn wildcard_matches_any_single_heading() {
        assert!(run("# Title\n\n## Whatever\n", &["# Title", "*"]).is_empty());
    }

    #[test]
    fn flags_a_mismatched_structure() {
        let diagnostics = run("# Title\n", &["# Title", "## Overview"]);
        assert_eq!(diagnostics.len(), 1);
    }
}
