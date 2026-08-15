//! Built-in lint rules.
//!
//! Each rule inspects the `mq-markdown` AST (and, where the AST alone doesn't preserve enough
//! information — whitespace, exact heading/list marker syntax — the raw source text) directly,
//! rather than running a user-supplied mq query. Accepting arbitrary mq expressions as rules is
//! a later stage of this project (see the crate-level docs).
//! [`RuleId::selector`](crate::RuleId::selector) still names the mq selector a rule primarily
//! corresponds to, where one applies.

mod blanks_around_fences;
mod blanks_around_headings;
mod blanks_around_lists;
mod blanks_around_tables;
mod code_block_style;
mod code_fence_style;
mod commands_show_output;
mod descriptive_link_text;
mod emphasis_style;
mod fenced_code_language;
mod first_line_heading;
mod heading_hierarchy_skip;
mod heading_start_left;
mod heading_style;
mod hr_style;
mod image_missing_alt;
mod line_length;
mod link_fragments;
mod link_image_reference_definitions;
mod link_image_style;
mod list_indent;
mod list_marker_space;
mod missing_front_matter_key;
mod no_bare_urls;
mod no_blanks_blockquote;
mod no_duplicate_heading;
mod no_emphasis_as_heading;
mod no_empty_links;
mod no_hard_tabs;
mod no_inline_html;
mod no_missing_space_atx;
mod no_missing_space_closed_atx;
mod no_multiple_blanks;
mod no_multiple_space_atx;
mod no_multiple_space_blockquote;
mod no_multiple_space_closed_atx;
mod no_reversed_links;
mod no_space_in_code;
mod no_space_in_emphasis;
mod no_space_in_links;
mod no_trailing_punctuation_heading;
mod no_trailing_spaces;
mod ol_prefix;
mod proper_names;
mod reference_links_images;
mod relative_link_exists;
mod required_headings;
mod single_h1;
mod single_trailing_newline;
mod strong_style;
mod table_column_count;
mod table_column_style;
mod table_pipe_style;
mod ul_indent;
mod ul_style;

use crate::{Diagnostic, LintConfig, RuleId, Severity};

/// A single built-in lint rule.
pub trait Rule: Send + Sync {
    /// Unique identifier for this rule.
    fn id(&self) -> RuleId;

    /// Default severity when the rule fires and no config override applies.
    fn default_severity(&self) -> Severity;

    /// Analyzes the parsed document and returns any diagnostics.
    ///
    /// `source` is the raw text `doc` was parsed from. Implementations set
    /// [`Diagnostic::severity`] to `self.default_severity()`; [`Linter`](crate::Linter) applies
    /// any configured override afterwards, so rules don't need to consult `config` for severity
    /// themselves. `path` is the linted file's own path, if it has one on disk (`None` for stdin).
    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        config: &LintConfig,
        path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic>;

    /// Rule-specific keys this rule reads from its `[rules.<id>]` config table via
    /// [`crate::config::RuleOptions`]'s `get_*` accessors (besides the universal `enabled`/
    /// `severity`, which every rule gets for free and never appear here). Config loading checks
    /// every key actually present in a rule's table against this list and rejects an unknown one
    /// as a typo (e.g. `limt` instead of `limit`) rather than silently ignoring it. Empty by
    /// default, for the many rules with no options at all.
    fn option_keys(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether this rule ever populates a diagnostic's [`crate::Fix`] (not necessarily every
    /// time it fires — see [`crate::Diagnostic::fix`]). Defaults to `true`, the more common case;
    /// override with `false` for a rule that can only report (see any rule file with no
    /// `.with_fix(...)` call in its `check()` for examples). Purely descriptive — backs
    /// `--list-rules`/`--explain` and has no effect on `--fix`'s own behavior, which already
    /// only applies whatever `Fix`es individual diagnostics happen to carry.
    fn fixable(&self) -> bool {
        true
    }
}

/// Returns the full built-in rule set, in a stable order matching [`RuleId::ALL`].
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(heading_hierarchy_skip::HeadingHierarchySkip),
        Box::new(image_missing_alt::ImageMissingAlt),
        Box::new(missing_front_matter_key::MissingFrontMatterKey),
        Box::new(heading_style::HeadingStyle),
        Box::new(no_missing_space_atx::NoMissingSpaceAtx),
        Box::new(no_multiple_space_atx::NoMultipleSpaceAtx),
        Box::new(no_missing_space_closed_atx::NoMissingSpaceClosedAtx),
        Box::new(no_multiple_space_closed_atx::NoMultipleSpaceClosedAtx),
        Box::new(blanks_around_headings::BlanksAroundHeadings),
        Box::new(heading_start_left::HeadingStartLeft),
        Box::new(no_duplicate_heading::NoDuplicateHeading),
        Box::new(single_h1::SingleH1),
        Box::new(no_trailing_punctuation_heading::NoTrailingPunctuationHeading),
        Box::new(no_emphasis_as_heading::NoEmphasisAsHeading),
        Box::new(first_line_heading::FirstLineHeading),
        Box::new(required_headings::RequiredHeadings),
        Box::new(ul_style::UlStyle),
        Box::new(list_indent::ListIndent),
        Box::new(ul_indent::UlIndent),
        Box::new(ol_prefix::OlPrefix),
        Box::new(list_marker_space::ListMarkerSpace),
        Box::new(blanks_around_lists::BlanksAroundLists),
        Box::new(no_trailing_spaces::NoTrailingSpaces),
        Box::new(no_hard_tabs::NoHardTabs),
        Box::new(no_multiple_blanks::NoMultipleBlanks),
        Box::new(line_length::LineLength),
        Box::new(no_multiple_space_blockquote::NoMultipleSpaceBlockquote),
        Box::new(no_blanks_blockquote::NoBlanksBlockquote),
        Box::new(single_trailing_newline::SingleTrailingNewline),
        Box::new(fenced_code_language::FencedCodeLanguage),
        Box::new(code_block_style::CodeBlockStyle),
        Box::new(code_fence_style::CodeFenceStyle),
        Box::new(blanks_around_fences::BlanksAroundFences),
        Box::new(no_bare_urls::NoBareUrls),
        Box::new(no_reversed_links::NoReversedLinks),
        Box::new(no_empty_links::NoEmptyLinks),
        Box::new(reference_links_images::ReferenceLinksImages),
        Box::new(link_image_reference_definitions::LinkImageReferenceDefinitions),
        Box::new(link_image_style::LinkImageStyle),
        Box::new(link_fragments::LinkFragments),
        Box::new(relative_link_exists::RelativeLinkExists),
        Box::new(descriptive_link_text::DescriptiveLinkText),
        Box::new(no_space_in_links::NoSpaceInLinks),
        Box::new(no_space_in_emphasis::NoSpaceInEmphasis),
        Box::new(no_space_in_code::NoSpaceInCode),
        Box::new(emphasis_style::EmphasisStyle),
        Box::new(strong_style::StrongStyle),
        Box::new(no_inline_html::NoInlineHtml),
        Box::new(proper_names::ProperNames),
        Box::new(commands_show_output::CommandsShowOutput),
        Box::new(hr_style::HrStyle),
        Box::new(table_pipe_style::TablePipeStyle),
        Box::new(table_column_count::TableColumnCount),
        Box::new(blanks_around_tables::BlanksAroundTables),
        Box::new(table_column_style::TableColumnStyle),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_matches_rule_id_all() {
        let rules = all_rules();
        assert_eq!(rules.len(), RuleId::ALL.len());
        for (rule, id) in rules.iter().zip(RuleId::ALL) {
            assert_eq!(rule.id(), *id);
        }
    }
}
