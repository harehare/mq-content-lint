//! Flags images (and image references) with empty alt text — the same accessibility check as
//! mq's own cookbook query `select(.image.alt == "")`, run here as a built-in rule instead of a
//! user-supplied query.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct ImageMissingAlt;

impl Rule for ImageMissingAlt {
    fn id(&self) -> RuleId {
        RuleId::ImageMissingAlt
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, doc: &mq_markdown::Markdown, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let (alt, url, position) = match node {
                Node::Image(image) => (image.alt.as_str(), image.url.as_str(), &image.position),
                Node::ImageRef(image_ref) => (image_ref.alt.as_str(), image_ref.ident.as_str(), &image_ref.position),
                _ => return,
            };

            if alt.trim().is_empty() {
                let mut diagnostic = Diagnostic::new(
                    LintMessage::ImageMissingAlt { url: url.to_string() },
                    self.default_severity(),
                );
                if let Some(position) = position.clone() {
                    diagnostic = diagnostic.with_range(position);
                }
                diagnostics.push(diagnostic);
            }
        });

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        ImageMissingAlt.check(&doc, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_when_alt_text_is_present() {
        assert!(run("![A cute cat](cat.png)\n").is_empty());
    }

    #[test]
    fn flags_empty_alt_text() {
        let diagnostics = run("![](missing-alt.png)\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::ImageMissingAlt {
                url: "missing-alt.png".to_string()
            }
        );
    }

    #[test]
    fn flags_whitespace_only_alt_text() {
        assert_eq!(run("![ ](missing-alt.png)\n").len(), 1);
    }

    #[test]
    fn finds_images_nested_in_links_and_tables() {
        assert_eq!(run("[![]()](https://example.com)\n").len(), 1);
        assert_eq!(run("| A |\n|---|\n| ![]() |\n").len(), 1);
    }

    #[test]
    fn checks_image_references_too() {
        let diagnostics = run("![][ref]\n\n[ref]: img.png\n");
        assert_eq!(diagnostics.len(), 1);
    }
}
