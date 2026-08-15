//! MD051: a same-document link (`[text](#section)`) whose fragment doesn't match any heading's
//! generated anchor slug. Slugs are computed GitHub-style (lowercased, punctuation stripped,
//! spaces to hyphens, duplicates suffixed `-1`, `-2`, ...). Not auto-fixable — the correct
//! fragment or heading text isn't something this rule can infer.

use std::collections::HashMap;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct LinkFragments;

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            slug.push('-');
        }
    }
    slug
}

fn collect_slugs(doc: &mq_markdown::Markdown) -> Vec<String> {
    let mut headings = Vec::new();
    crate::walk::walk(doc.nodes.iter(), &mut |node| {
        if let Node::Heading(_) = node {
            headings.push(slugify(&node.value()));
        }
    });

    let mut counts: HashMap<String, usize> = HashMap::new();
    headings
        .into_iter()
        .map(|slug| {
            let count = counts.entry(slug.clone()).or_insert(0);
            let result = if *count == 0 {
                slug.clone()
            } else {
                format!("{slug}-{count}")
            };
            *count += 1;
            result
        })
        .collect()
}

impl Rule for LinkFragments {
    fn id(&self) -> RuleId {
        RuleId::LinkFragments
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        _source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let slugs = collect_slugs(doc);
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Link(link) = node else { return };
            let Some(fragment) = link.url.as_str().strip_prefix('#') else {
                return;
            };
            if fragment.is_empty() || slugs.iter().any(|s| s == fragment) {
                return;
            }
            let mut diagnostic = Diagnostic::new(
                LintMessage::LinkFragments {
                    fragment: fragment.to_string(),
                },
                self.default_severity(),
            );
            if let Some(position) = link.position.clone() {
                diagnostic = diagnostic.with_range(position);
            }
            diagnostics.push(diagnostic);
        });
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        LinkFragments.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_for_a_matching_fragment() {
        assert!(run("# Getting Started\n\n[link](#getting-started)\n").is_empty());
    }

    #[test]
    fn flags_a_fragment_with_no_matching_heading() {
        assert_eq!(run("# Title\n\n[link](#nope)\n").len(), 1);
    }

    #[test]
    fn does_not_flag_non_fragment_links() {
        assert!(run("[link](https://example.com)\n").is_empty());
    }
}
