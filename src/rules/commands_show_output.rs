//! MD014: every line in a fenced code block prefixed with `$ ` (shell prompt), with none showing
//! the command's actual output — usually a sign the `$ ` prefixes should just be removed, since
//! they add noise without adding information.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct CommandsShowOutput;

impl Rule for CommandsShowOutput {
    fn id(&self) -> RuleId {
        RuleId::CommandsShowOutput
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, doc: &mq_markdown::Markdown, _source: &str, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Code(code) = node else { return };
            let content_lines: Vec<&str> = code.value.lines().filter(|l| !l.trim().is_empty()).collect();
            if content_lines.is_empty() || !content_lines.iter().all(|l| l.trim_start().starts_with("$ ")) {
                return;
            }

            let mut diagnostic = Diagnostic::new(LintMessage::CommandsShowOutput, self.default_severity());
            if let Some(position) = &code.position {
                diagnostic = diagnostic.with_range(position.clone());
                let content_start = position.start.line + 1;
                let content_end = position.end.line.saturating_sub(1);
                if content_end >= content_start {
                    let stripped: String = code
                        .value
                        .lines()
                        .map(|l| l.strip_prefix("$ ").unwrap_or(l))
                        .collect::<Vec<_>>()
                        .join("\n");
                    diagnostic = diagnostic.with_fix(Fix::new(
                        Range {
                            start_line: content_start,
                            start_column: 1,
                            end_line: content_end + 1,
                            end_column: 1,
                        },
                        format!("{stripped}\n"),
                    ));
                }
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
        CommandsShowOutput.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_when_output_is_shown() {
        assert!(run("```\n$ ls\nfile.txt\n```\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_block_of_only_commands() {
        let source = "```\n$ ls\n$ pwd\n```\n";
        let diagnostics = run(source);
        assert_eq!(diagnostics.len(), 1);
        let fixed = crate::fix::apply_fixes(source, &[diagnostics[0].fix.clone().unwrap()]);
        assert_eq!(fixed, "```\nls\npwd\n```\n");
    }
}
