//! MD029: ordered list item prefixes should be consistent — either all the same number
//! (`1. / 1. / 1.`) or strictly incrementing (`1. / 2. / 3.`). `[rules.ol_prefix] style` accepts
//! `"one_or_ordered"` (default — infer the intended pattern from the second item), `"one"`, or
//! `"ordered"`.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct OlPrefix;

/// Parses the leading number and its `.`/`)` delimiter from a list item's raw line, returning
/// `(number, delimiter_char, digit_char_count)`.
fn parse_prefix(line: &str) -> Option<(u32, char, usize)> {
    let trimmed = line.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let delimiter = trimmed.chars().nth(digits.len())?;
    if delimiter != '.' && delimiter != ')' {
        return None;
    }
    Some((digits.parse().ok()?, delimiter, digits.len()))
}

impl Rule for OlPrefix {
    fn id(&self) -> RuleId {
        RuleId::OlPrefix
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
        let forced_style = config.rule_options(self.id()).get_str("style").map(str::to_string);
        let mut diagnostics = Vec::new();

        let mut i = 0;
        while i < doc.nodes.len() {
            let Node::List(first) = &doc.nodes[i] else {
                i += 1;
                continue;
            };
            if !first.ordered {
                i += 1;
                continue;
            }
            let level = first.level;
            let mut j = i;
            let mut items = Vec::new();
            while let Some(Node::List(list)) = doc.nodes.get(j) {
                if !list.ordered || list.level != level {
                    break;
                }
                if let Some(position) = &list.position
                    && let Some((_, line)) =
                        crate::text::numbered_lines(source).find(|(n, _)| *n == position.start.line)
                    && let Some(prefix) = parse_prefix(line)
                {
                    items.push((position.start.line, prefix));
                }
                j += 1;
            }

            if items.len() >= 2 {
                let use_ordered = match forced_style.as_deref() {
                    Some("one") => false,
                    Some("ordered") => true,
                    _ => items[1].1.0 != items[0].1.0,
                };
                let first_number = items[0].1.0;

                for (idx, (line, (number, delimiter, digit_len))) in items.iter().enumerate() {
                    let expected = if use_ordered {
                        first_number + idx as u32
                    } else {
                        first_number
                    };
                    if *number != expected {
                        let range = Range::single_line(*line, 1, digit_len + 1);
                        diagnostics.push(
                            Diagnostic::new(
                                LintMessage::OlPrefix {
                                    expected: format!("{expected}{delimiter}"),
                                    found: format!("{number}{delimiter}"),
                                },
                                self.default_severity(),
                            )
                            .with_range(range)
                            .with_fix(Fix::new(range, expected.to_string())),
                        );
                    }
                }
            }

            i = j.max(i + 1);
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(markdown: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        OlPrefix.check(&doc, markdown, &LintConfig::default())
    }

    #[test]
    fn no_diagnostics_for_all_ones() {
        assert!(run("1. one\n1. two\n1. three\n").is_empty());
    }

    #[test]
    fn no_diagnostics_for_sequential_numbers() {
        assert!(run("1. one\n2. two\n3. three\n").is_empty());
    }

    #[test]
    fn flags_a_broken_sequential_list() {
        let diagnostics = run("1. one\n2. two\n4. three\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "3");
    }

    #[test]
    fn single_item_lists_are_never_flagged() {
        assert!(run("5. only item\n").is_empty());
    }
}
