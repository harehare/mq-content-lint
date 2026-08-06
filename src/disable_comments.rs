//! Inline HTML comments that suppress diagnostics for a region of the source, independent of
//! `mq-content-lint.toml` — the same escape hatch markdownlint's `<!-- markdownlint-disable -->`
//! provides, so a single false positive doesn't force a project-wide config change.
//!
//! Four directives, each its own HTML comment on its own line (leading/trailing whitespace on
//! the line is fine; anything else sharing the line is not — this is a deliberately narrow,
//! easy-to-spot syntax rather than a general in-line marker):
//!
//! - `<!-- mq-content-lint-disable [RULE_ID, ...] -->` — suppress the named rules (or every rule,
//!   if none are named) from this line onward, until a matching `-enable`.
//! - `<!-- mq-content-lint-enable [RULE_ID, ...] -->` — re-enable the named rules (or every rule).
//! - `<!-- mq-content-lint-disable-line [RULE_ID, ...] -->` — suppress only on the line the
//!   comment itself is on.
//! - `<!-- mq-content-lint-disable-next-line [RULE_ID, ...] -->` — suppress only on the following
//!   line.
//!
//! `RULE_ID` is a comma-separated list of built-in rule ids or custom rule ids; a diagnostic is
//! suppressed by matching against [`crate::report_item::ReportItem::rule_id`], so both kinds work
//! identically. Applied once, after both built-in and custom rules have run — see
//! [`crate::report_item::lint`] — rather than duplicated into each rule, so every rule (present
//! and future) gets this behavior for free.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
struct LineState {
    all: bool,
    rules: HashSet<String>,
}

impl LineState {
    fn suppresses(&self, rule_id: &str) -> bool {
        self.all || self.rules.contains(rule_id)
    }
}

/// Per-line suppression state derived from a document's inline disable comments. Built once per
/// lint pass via [`scan`] and consulted once per diagnostic.
pub(crate) struct DisabledLines {
    per_line: HashMap<usize, LineState>,
}

impl DisabledLines {
    /// Whether a diagnostic from `rule_id`, reported at `line` (1-based), should be suppressed.
    pub(crate) fn suppresses(&self, rule_id: &str, line: usize) -> bool {
        self.per_line.get(&line).is_some_and(|state| state.suppresses(rule_id))
    }
}

enum Directive {
    Disable(Vec<String>),
    Enable(Vec<String>),
    DisableLine(Vec<String>),
    DisableNextLine(Vec<String>),
}

/// Parses a directive out of `line` if the entire trimmed line is one of the four recognized HTML
/// comments; anything else (ordinary content, a comment that also carries other text, an
/// unrecognized directive name) is not a directive.
fn parse_directive(line: &str) -> Option<Directive> {
    let inner = line.trim().strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let (name, rest) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
    let rules: Vec<String> = rest
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    match name {
        "mq-content-lint-disable" => Some(Directive::Disable(rules)),
        "mq-content-lint-enable" => Some(Directive::Enable(rules)),
        "mq-content-lint-disable-line" => Some(Directive::DisableLine(rules)),
        "mq-content-lint-disable-next-line" => Some(Directive::DisableNextLine(rules)),
        _ => None,
    }
}

fn suppress(entry: &mut LineState, rules: Vec<String>) {
    if rules.is_empty() {
        entry.all = true;
    } else {
        entry.rules.extend(rules);
    }
}

/// Scans `source` for disable/enable directives, returning the per-line suppression state they
/// describe.
pub(crate) fn scan(source: &str) -> DisabledLines {
    let mut per_line: HashMap<usize, LineState> = HashMap::new();
    let mut block_all = false;
    let mut block_rules: HashSet<String> = HashSet::new();

    for (line_number, line) in crate::text::numbered_lines(source) {
        match parse_directive(line) {
            Some(Directive::Disable(rules)) if rules.is_empty() => block_all = true,
            Some(Directive::Disable(rules)) => block_rules.extend(rules),
            Some(Directive::Enable(rules)) if rules.is_empty() => {
                block_all = false;
                block_rules.clear();
            }
            Some(Directive::Enable(rules)) => block_rules.retain(|r| !rules.contains(r)),
            Some(Directive::DisableLine(rules)) => suppress(per_line.entry(line_number).or_default(), rules),
            Some(Directive::DisableNextLine(rules)) => suppress(per_line.entry(line_number + 1).or_default(), rules),
            None => {}
        }

        if block_all || !block_rules.is_empty() {
            let entry = per_line.entry(line_number).or_default();
            entry.all |= block_all;
            entry.rules.extend(block_rules.iter().cloned());
        }
    }

    DisabledLines { per_line }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_with_no_directives_is_unaffected() {
        let disabled = scan("# Title\n\nSome text.\n");
        assert!(!disabled.suppresses("any_rule", 1));
        assert!(!disabled.suppresses("any_rule", 3));
    }

    #[test]
    fn disable_suppresses_a_named_rule_from_that_line_onward() {
        let source = "line 1\n<!-- mq-content-lint-disable line_length -->\nline 3\nline 4\n";
        let disabled = scan(source);
        assert!(!disabled.suppresses("line_length", 1));
        assert!(disabled.suppresses("line_length", 2));
        assert!(disabled.suppresses("line_length", 3));
        assert!(disabled.suppresses("line_length", 4));
        assert!(!disabled.suppresses("no_hard_tabs", 3));
    }

    #[test]
    fn disable_with_no_rules_suppresses_everything() {
        let source = "<!-- mq-content-lint-disable -->\nline 2\n";
        let disabled = scan(source);
        assert!(disabled.suppresses("line_length", 2));
        assert!(disabled.suppresses("literally_anything", 2));
    }

    #[test]
    fn enable_ends_a_disabled_span() {
        let source = "<!-- mq-content-lint-disable line_length -->\nline 2\n<!-- mq-content-lint-enable line_length -->\nline 4\n";
        let disabled = scan(source);
        assert!(disabled.suppresses("line_length", 2));
        assert!(!disabled.suppresses("line_length", 4));
    }

    #[test]
    fn enable_with_no_rules_clears_a_disable_all() {
        let source = "<!-- mq-content-lint-disable -->\n<!-- mq-content-lint-enable -->\nline 3\n";
        let disabled = scan(source);
        assert!(!disabled.suppresses("anything", 3));
    }

    #[test]
    fn disable_line_only_affects_its_own_line() {
        let source = "line 1\n<!-- mq-content-lint-disable-line line_length -->\nline 3\n";
        let disabled = scan(source);
        assert!(!disabled.suppresses("line_length", 1));
        assert!(disabled.suppresses("line_length", 2));
        assert!(!disabled.suppresses("line_length", 3));
    }

    #[test]
    fn disable_next_line_only_affects_the_following_line() {
        let source = "<!-- mq-content-lint-disable-next-line line_length -->\nline 2\nline 3\n";
        let disabled = scan(source);
        assert!(!disabled.suppresses("line_length", 1));
        assert!(disabled.suppresses("line_length", 2));
        assert!(!disabled.suppresses("line_length", 3));
    }

    #[test]
    fn multiple_rule_ids_are_comma_separated() {
        let source = "<!-- mq-content-lint-disable-line line_length, no_hard_tabs -->\n";
        let disabled = scan(source);
        assert!(disabled.suppresses("line_length", 1));
        assert!(disabled.suppresses("no_hard_tabs", 1));
        assert!(!disabled.suppresses("no_trailing_spaces", 1));
    }

    #[test]
    fn a_comment_sharing_its_line_with_other_text_is_not_a_directive() {
        let source = "text <!-- mq-content-lint-disable line_length --> more text\n";
        let disabled = scan(source);
        assert!(!disabled.suppresses("line_length", 1));
    }

    #[test]
    fn an_unrecognized_comment_is_ignored() {
        let disabled = scan("<!-- just a regular comment -->\n");
        assert!(!disabled.suppresses("anything", 1));
    }
}
