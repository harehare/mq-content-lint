//! MD033: raw inline/block HTML. `[rules.no_inline_html] allowed` lists tag names (lowercase, no
//! brackets) that are permitted anyway, e.g. `allowed = ["br", "img"]`. Not auto-fixable —
//! converting arbitrary HTML to Markdown isn't always possible.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct NoInlineHtml;

/// Extracts the tag name from a raw HTML fragment like `<div class="x">` or `</div>`.
fn tag_name(html: &str) -> Option<String> {
    let trimmed = html.trim_start_matches('<').trim_start_matches('/');
    let name: String = trimmed
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    (!name.is_empty()).then(|| name.to_lowercase())
}

impl Rule for NoInlineHtml {
    fn id(&self) -> RuleId {
        RuleId::NoInlineHtml
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let allowed = config
            .rule_options(self.id())
            .get_str_array("allowed")
            .unwrap_or_default();
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Html(html) = node else { return };
            let Some(tag) = tag_name(&html.value) else { return };
            if allowed.iter().any(|a| a.eq_ignore_ascii_case(&tag)) {
                return;
            }
            let mut diagnostic = Diagnostic::new(LintMessage::NoInlineHtml { tag }, self.default_severity());
            if let Some(position) = html.position.clone() {
                diagnostic = diagnostic.with_range(position);
            }
            diagnostics.push(diagnostic);
        });
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["allowed"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        NoInlineHtml.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_without_html() {
        assert!(run("Plain **markdown**.\n").is_empty());
    }

    #[test]
    fn flags_a_raw_html_tag() {
        // A block-level HTML element parses as a single `Html` node spanning the whole block.
        assert_eq!(run("<div>content</div>\n").len(), 1);
    }

    #[test]
    fn allowed_tags_are_not_flagged() {
        let config = LintConfig::from_toml_str("[rules.no_inline_html]\nallowed = [\"br\"]\n").unwrap();
        let doc: mq_markdown::Markdown = "line one<br>line two\n".parse().unwrap();
        assert!(NoInlineHtml.check(&doc, "line one<br>line two\n", &config).is_empty());
    }
}
