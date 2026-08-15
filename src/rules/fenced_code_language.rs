//! MD040: a fenced code block should specify a language (` ```rust` rather than bare ` ``` `),
//! so syntax highlighters and screen readers know what to do with it. Not auto-fixable — there's
//! no way to guess the language from the code alone.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct FencedCodeLanguage;

impl Rule for FencedCodeLanguage {
    fn id(&self) -> RuleId {
        RuleId::FencedCodeLanguage
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
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
        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            if let Node::Code(code) = node
                && code.fence
                && code.lang.as_deref().unwrap_or("").is_empty()
            {
                let mut diagnostic = Diagnostic::new(LintMessage::FencedCodeLanguage, self.default_severity());
                if let Some(position) = code.position.clone() {
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
        FencedCodeLanguage.check(&doc, markdown, &LintConfig::default(), None)
    }

    #[test]
    fn no_diagnostics_when_language_is_specified() {
        assert!(run("```rust\nfn main() {}\n```\n").is_empty());
    }

    #[test]
    fn flags_a_fenced_block_with_no_language() {
        assert_eq!(run("```\nsome code\n```\n").len(), 1);
    }

    #[test]
    fn does_not_flag_indented_code_blocks() {
        assert!(run("    indented code\n").is_empty());
    }
}
