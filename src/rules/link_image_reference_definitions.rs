//! MD053: a `[label]: url` reference definition that no `[text][label]` link or image in the
//! document actually uses. Fixable by deleting the unused definition line.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct LinkImageReferenceDefinitions;

impl Rule for LinkImageReferenceDefinitions {
    fn id(&self) -> RuleId {
        RuleId::LinkImageReferenceDefinitions
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        _config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let mut used = std::collections::HashSet::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let ident = match node {
                Node::LinkRef(link_ref) => Some(link_ref.ident.to_lowercase()),
                Node::ImageRef(image_ref) => Some(image_ref.ident.to_lowercase()),
                _ => None,
            };
            if let Some(ident) = ident {
                used.insert(ident);
            }
        });

        let total_lines = source.lines().count();
        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Definition(def) = node else { return };
            if used.contains(&def.ident.to_lowercase()) {
                return;
            }
            let mut diagnostic = Diagnostic::new(
                LintMessage::LinkImageReferenceDefinitions {
                    label: def.label.clone().unwrap_or_else(|| def.ident.clone()),
                },
                self.default_severity(),
            );
            if let Some(position) = &def.position {
                diagnostic = diagnostic.with_range(position.clone());
                let end_line = position.end.line;
                let fix_range = Range {
                    start_line: position.start.line,
                    start_column: 1,
                    end_line: (end_line + 1).min(total_lines + 1),
                    end_column: 1,
                };
                diagnostic = diagnostic.with_fix(Fix::new(fix_range, ""));
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
        LinkImageReferenceDefinitions.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_definition_is_used() {
        assert!(run("[text][ref]\n\n[ref]: https://example.com\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_an_unused_definition() {
        let source = "Body text.\n\n[unused]: https://example.com\n";
        let diagnostics = run(source);
        assert_eq!(diagnostics.len(), 1);
        let fixed = crate::fix::apply_fixes(source, &[diagnostics[0].fix.clone().unwrap()]);
        assert_eq!(fixed, "Body text.\n\n");
    }
}
