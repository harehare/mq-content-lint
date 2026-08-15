//! Rule identity and diagnostic message types.
//!
//! Every lint rule is identified by a [`RuleId`] variant and, when it fires, produces a
//! [`LintMessage`] carrying whatever data is needed to render the diagnostic text. Keeping both
//! as enums (rather than free-form strings) means the compiler enforces that every rule has
//! exactly one ID and that every message variant maps to a real rule.
//!
//! Rule IDs and short descriptions below are cross-referenced against their
//! [markdownlint](https://github.com/DavidAnson/markdownlint) equivalent (`MDxxx`) in doc
//! comments for readers coming from that tool; this crate's rule ids are its own `snake_case`
//! names, not markdownlint's numbers.

use std::fmt;
use std::str::FromStr;

/// Unique identifier for a built-in lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleId {
    /// MD001: heading levels should only increment by one level at a time.
    HeadingHierarchySkip,
    /// MD045: images should have alternate text (alt text).
    ImageMissingAlt,
    /// Required front matter keys (not a markdownlint rule).
    MissingFrontMatterKey,
    /// MD003: heading style should be consistent (atx, atx_closed, or setext).
    HeadingStyle,
    /// MD018: no space after `#` on an ATX-style heading.
    NoMissingSpaceAtx,
    /// MD019: multiple spaces after `#` on an ATX-style heading.
    NoMultipleSpaceAtx,
    /// MD020: no space inside the hashes on a closed ATX-style heading.
    NoMissingSpaceClosedAtx,
    /// MD021: multiple spaces inside the hashes on a closed ATX-style heading.
    NoMultipleSpaceClosedAtx,
    /// MD022: headings should be surrounded by blank lines.
    BlanksAroundHeadings,
    /// MD023: headings must start at the beginning of the line.
    HeadingStartLeft,
    /// MD024: multiple headings with the same text.
    NoDuplicateHeading,
    /// MD025: multiple top-level (h1) headings in the same document.
    SingleH1,
    /// MD026: trailing punctuation in a heading.
    NoTrailingPunctuationHeading,
    /// MD036: emphasis used instead of a heading.
    NoEmphasisAsHeading,
    /// MD041: the first line in a file should be a top-level heading.
    FirstLineHeading,
    /// MD043: the document's headings should match a required structure.
    RequiredHeadings,
    /// MD004: unordered list style should be consistent.
    UlStyle,
    /// MD005: inconsistent indentation for list items at the same level.
    ListIndent,
    /// MD007: unordered list indentation.
    UlIndent,
    /// MD029: ordered list item prefixes should be consistent.
    OlPrefix,
    /// MD030: spaces after list markers.
    ListMarkerSpace,
    /// MD032: lists should be surrounded by blank lines.
    BlanksAroundLists,
    /// MD009: trailing spaces.
    NoTrailingSpaces,
    /// MD010: hard tabs.
    NoHardTabs,
    /// MD012: multiple consecutive blank lines.
    NoMultipleBlanks,
    /// MD013: line length.
    LineLength,
    /// MD027: multiple spaces after the blockquote symbol.
    NoMultipleSpaceBlockquote,
    /// MD028: blank line inside a blockquote.
    NoBlanksBlockquote,
    /// MD047: files should end with exactly one trailing newline.
    SingleTrailingNewline,
    /// MD040: fenced code blocks should have a language specified.
    FencedCodeLanguage,
    /// MD046: code block style should be consistent (fenced vs. indented).
    CodeBlockStyle,
    /// MD048: code fence style should be consistent (backtick vs. tilde).
    CodeFenceStyle,
    /// MD031: fenced code blocks should be surrounded by blank lines.
    BlanksAroundFences,
    /// MD034: bare URL used without angle brackets or link syntax.
    NoBareUrls,
    /// MD011: reversed link syntax, e.g. `(text)[url]` instead of `[text](url)`.
    NoReversedLinks,
    /// MD042: links with no destination or only a placeholder like `#`.
    NoEmptyLinks,
    /// MD052: a reference link/image uses a label with no matching definition.
    ReferenceLinksImages,
    /// MD053: a link/image reference definition is never used.
    LinkImageReferenceDefinitions,
    /// MD054: link/image style should be consistent (inline, reference, autolink, ...).
    LinkImageStyle,
    /// MD051: a link fragment (`#section`) doesn't match any heading in the document.
    LinkFragments,
    /// MD057: a relative link doesn't point to a file that exists on disk.
    RelativeLinkExists,
    /// MD059: link text should be descriptive, not generic like "click here".
    DescriptiveLinkText,
    /// MD039: spaces inside link text brackets.
    NoSpaceInLinks,
    /// MD037: spaces inside emphasis markers.
    NoSpaceInEmphasis,
    /// MD038: spaces inside code span backticks.
    NoSpaceInCode,
    /// MD049: emphasis style should be consistent (`*text*` vs. `_text_`).
    EmphasisStyle,
    /// MD050: strong style should be consistent (`**text**` vs. `__text__`).
    StrongStyle,
    /// MD033: inline HTML.
    NoInlineHtml,
    /// MD044: proper names should use the configured capitalization.
    ProperNames,
    /// MD014: `$` shown before commands with no output shown.
    CommandsShowOutput,
    /// MD035: horizontal rule style should be consistent.
    HrStyle,
    /// MD055: table pipe style should be consistent (leading/trailing pipes).
    TablePipeStyle,
    /// MD056: every row in a table should have the same number of cells as the header.
    TableColumnCount,
    /// MD058: tables should be surrounded by blank lines.
    BlanksAroundTables,
    /// MD060: table column style (padding around `|`) should be consistent.
    TableColumnStyle,
}

impl RuleId {
    /// All known rule IDs, in a stable order.
    pub const ALL: &'static [RuleId] = &[
        RuleId::HeadingHierarchySkip,
        RuleId::ImageMissingAlt,
        RuleId::MissingFrontMatterKey,
        RuleId::HeadingStyle,
        RuleId::NoMissingSpaceAtx,
        RuleId::NoMultipleSpaceAtx,
        RuleId::NoMissingSpaceClosedAtx,
        RuleId::NoMultipleSpaceClosedAtx,
        RuleId::BlanksAroundHeadings,
        RuleId::HeadingStartLeft,
        RuleId::NoDuplicateHeading,
        RuleId::SingleH1,
        RuleId::NoTrailingPunctuationHeading,
        RuleId::NoEmphasisAsHeading,
        RuleId::FirstLineHeading,
        RuleId::RequiredHeadings,
        RuleId::UlStyle,
        RuleId::ListIndent,
        RuleId::UlIndent,
        RuleId::OlPrefix,
        RuleId::ListMarkerSpace,
        RuleId::BlanksAroundLists,
        RuleId::NoTrailingSpaces,
        RuleId::NoHardTabs,
        RuleId::NoMultipleBlanks,
        RuleId::LineLength,
        RuleId::NoMultipleSpaceBlockquote,
        RuleId::NoBlanksBlockquote,
        RuleId::SingleTrailingNewline,
        RuleId::FencedCodeLanguage,
        RuleId::CodeBlockStyle,
        RuleId::CodeFenceStyle,
        RuleId::BlanksAroundFences,
        RuleId::NoBareUrls,
        RuleId::NoReversedLinks,
        RuleId::NoEmptyLinks,
        RuleId::ReferenceLinksImages,
        RuleId::LinkImageReferenceDefinitions,
        RuleId::LinkImageStyle,
        RuleId::LinkFragments,
        RuleId::RelativeLinkExists,
        RuleId::DescriptiveLinkText,
        RuleId::NoSpaceInLinks,
        RuleId::NoSpaceInEmphasis,
        RuleId::NoSpaceInCode,
        RuleId::EmphasisStyle,
        RuleId::StrongStyle,
        RuleId::NoInlineHtml,
        RuleId::ProperNames,
        RuleId::CommandsShowOutput,
        RuleId::HrStyle,
        RuleId::TablePipeStyle,
        RuleId::TableColumnCount,
        RuleId::BlanksAroundTables,
        RuleId::TableColumnStyle,
    ];

    /// The rule's `snake_case` identifier, as used in config keys and CLI flags.
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleId::HeadingHierarchySkip => "heading_hierarchy_skip",
            RuleId::ImageMissingAlt => "image_missing_alt",
            RuleId::MissingFrontMatterKey => "missing_front_matter_key",
            RuleId::HeadingStyle => "heading_style",
            RuleId::NoMissingSpaceAtx => "no_missing_space_atx",
            RuleId::NoMultipleSpaceAtx => "no_multiple_space_atx",
            RuleId::NoMissingSpaceClosedAtx => "no_missing_space_closed_atx",
            RuleId::NoMultipleSpaceClosedAtx => "no_multiple_space_closed_atx",
            RuleId::BlanksAroundHeadings => "blanks_around_headings",
            RuleId::HeadingStartLeft => "heading_start_left",
            RuleId::NoDuplicateHeading => "no_duplicate_heading",
            RuleId::SingleH1 => "single_h1",
            RuleId::NoTrailingPunctuationHeading => "no_trailing_punctuation_heading",
            RuleId::NoEmphasisAsHeading => "no_emphasis_as_heading",
            RuleId::FirstLineHeading => "first_line_heading",
            RuleId::RequiredHeadings => "required_headings",
            RuleId::UlStyle => "ul_style",
            RuleId::ListIndent => "list_indent",
            RuleId::UlIndent => "ul_indent",
            RuleId::OlPrefix => "ol_prefix",
            RuleId::ListMarkerSpace => "list_marker_space",
            RuleId::BlanksAroundLists => "blanks_around_lists",
            RuleId::NoTrailingSpaces => "no_trailing_spaces",
            RuleId::NoHardTabs => "no_hard_tabs",
            RuleId::NoMultipleBlanks => "no_multiple_blanks",
            RuleId::LineLength => "line_length",
            RuleId::NoMultipleSpaceBlockquote => "no_multiple_space_blockquote",
            RuleId::NoBlanksBlockquote => "no_blanks_blockquote",
            RuleId::SingleTrailingNewline => "single_trailing_newline",
            RuleId::FencedCodeLanguage => "fenced_code_language",
            RuleId::CodeBlockStyle => "code_block_style",
            RuleId::CodeFenceStyle => "code_fence_style",
            RuleId::BlanksAroundFences => "blanks_around_fences",
            RuleId::NoBareUrls => "no_bare_urls",
            RuleId::NoReversedLinks => "no_reversed_links",
            RuleId::NoEmptyLinks => "no_empty_links",
            RuleId::ReferenceLinksImages => "reference_links_images",
            RuleId::LinkImageReferenceDefinitions => "link_image_reference_definitions",
            RuleId::LinkImageStyle => "link_image_style",
            RuleId::LinkFragments => "link_fragments",
            RuleId::RelativeLinkExists => "relative_link_exists",
            RuleId::DescriptiveLinkText => "descriptive_link_text",
            RuleId::NoSpaceInLinks => "no_space_in_links",
            RuleId::NoSpaceInEmphasis => "no_space_in_emphasis",
            RuleId::NoSpaceInCode => "no_space_in_code",
            RuleId::EmphasisStyle => "emphasis_style",
            RuleId::StrongStyle => "strong_style",
            RuleId::NoInlineHtml => "no_inline_html",
            RuleId::ProperNames => "proper_names",
            RuleId::CommandsShowOutput => "commands_show_output",
            RuleId::HrStyle => "hr_style",
            RuleId::TablePipeStyle => "table_pipe_style",
            RuleId::TableColumnCount => "table_column_count",
            RuleId::BlanksAroundTables => "blanks_around_tables",
            RuleId::TableColumnStyle => "table_column_style",
        }
    }

    /// The mq selector a rule primarily inspects, where one node type applies. `None` for rules
    /// that scan raw text/lines rather than a single node type (whitespace rules, line length,
    /// multi-line-span rules).
    pub fn selector(&self) -> Option<mq_lang::Selector> {
        use mq_lang::Selector;
        match self {
            RuleId::HeadingHierarchySkip
            | RuleId::HeadingStyle
            | RuleId::NoMissingSpaceAtx
            | RuleId::NoMultipleSpaceAtx
            | RuleId::NoMissingSpaceClosedAtx
            | RuleId::NoMultipleSpaceClosedAtx
            | RuleId::BlanksAroundHeadings
            | RuleId::HeadingStartLeft
            | RuleId::NoDuplicateHeading
            | RuleId::NoTrailingPunctuationHeading
            | RuleId::FirstLineHeading
            | RuleId::RequiredHeadings => Some(Selector::Heading(None)),
            RuleId::SingleH1 => Some(Selector::Heading(Some(1))),
            RuleId::ImageMissingAlt => Some(Selector::Image),
            RuleId::MissingFrontMatterKey => Some(Selector::Yaml),
            RuleId::NoEmphasisAsHeading | RuleId::EmphasisStyle | RuleId::NoSpaceInEmphasis => Some(Selector::Emphasis),
            RuleId::UlStyle
            | RuleId::ListIndent
            | RuleId::UlIndent
            | RuleId::OlPrefix
            | RuleId::ListMarkerSpace
            | RuleId::BlanksAroundLists => Some(Selector::List(None, None)),
            RuleId::NoMultipleSpaceBlockquote | RuleId::NoBlanksBlockquote => Some(Selector::Blockquote),
            RuleId::FencedCodeLanguage
            | RuleId::CodeBlockStyle
            | RuleId::CodeFenceStyle
            | RuleId::BlanksAroundFences => Some(Selector::Code),
            RuleId::CommandsShowOutput => Some(Selector::Code),
            RuleId::NoReversedLinks | RuleId::NoBareUrls | RuleId::ProperNames => Some(Selector::Text),
            RuleId::NoEmptyLinks
            | RuleId::LinkImageStyle
            | RuleId::LinkFragments
            | RuleId::RelativeLinkExists
            | RuleId::DescriptiveLinkText
            | RuleId::NoSpaceInLinks => Some(Selector::Link),
            RuleId::ReferenceLinksImages => Some(Selector::LinkRef),
            RuleId::LinkImageReferenceDefinitions => Some(Selector::Definition),
            RuleId::NoSpaceInCode => Some(Selector::InlineCode),
            RuleId::StrongStyle => Some(Selector::Strong),
            RuleId::NoInlineHtml => Some(Selector::Html),
            RuleId::HrStyle => Some(Selector::HorizontalRule),
            RuleId::TablePipeStyle
            | RuleId::TableColumnCount
            | RuleId::BlanksAroundTables
            | RuleId::TableColumnStyle => Some(Selector::Table(None, None)),
            RuleId::NoTrailingSpaces
            | RuleId::NoHardTabs
            | RuleId::NoMultipleBlanks
            | RuleId::LineLength
            | RuleId::SingleTrailingNewline => None,
        }
    }

    /// A one-line description of what the rule checks, cross-referenced against its
    /// [markdownlint](https://github.com/DavidAnson/markdownlint) equivalent (`MDxxx`) where one
    /// exists — the same text as this variant's doc comment, kept in sync by hand since doc
    /// comments aren't readable at runtime. Backs `mq-content-lint --explain <rule-id>`.
    pub fn description(&self) -> &'static str {
        match self {
            RuleId::HeadingHierarchySkip => "MD001: heading levels should only increment by one level at a time.",
            RuleId::ImageMissingAlt => "MD045: images should have alternate text (alt text).",
            RuleId::MissingFrontMatterKey => "Required front matter keys (not a markdownlint rule).",
            RuleId::HeadingStyle => "MD003: heading style should be consistent (atx, atx_closed, or setext).",
            RuleId::NoMissingSpaceAtx => "MD018: no space after `#` on an ATX-style heading.",
            RuleId::NoMultipleSpaceAtx => "MD019: multiple spaces after `#` on an ATX-style heading.",
            RuleId::NoMissingSpaceClosedAtx => "MD020: no space inside the hashes on a closed ATX-style heading.",
            RuleId::NoMultipleSpaceClosedAtx => {
                "MD021: multiple spaces inside the hashes on a closed ATX-style heading."
            }
            RuleId::BlanksAroundHeadings => "MD022: headings should be surrounded by blank lines.",
            RuleId::HeadingStartLeft => "MD023: headings must start at the beginning of the line.",
            RuleId::NoDuplicateHeading => "MD024: multiple headings with the same text.",
            RuleId::SingleH1 => "MD025: multiple top-level (h1) headings in the same document.",
            RuleId::NoTrailingPunctuationHeading => "MD026: trailing punctuation in a heading.",
            RuleId::NoEmphasisAsHeading => "MD036: emphasis used instead of a heading.",
            RuleId::FirstLineHeading => "MD041: the first line in a file should be a top-level heading.",
            RuleId::RequiredHeadings => "MD043: the document's headings should match a required structure.",
            RuleId::UlStyle => "MD004: unordered list style should be consistent.",
            RuleId::ListIndent => "MD005: inconsistent indentation for list items at the same level.",
            RuleId::UlIndent => "MD007: unordered list indentation.",
            RuleId::OlPrefix => "MD029: ordered list item prefixes should be consistent.",
            RuleId::ListMarkerSpace => "MD030: spaces after list markers.",
            RuleId::BlanksAroundLists => "MD032: lists should be surrounded by blank lines.",
            RuleId::NoTrailingSpaces => "MD009: trailing spaces.",
            RuleId::NoHardTabs => "MD010: hard tabs.",
            RuleId::NoMultipleBlanks => "MD012: multiple consecutive blank lines.",
            RuleId::LineLength => "MD013: line length.",
            RuleId::NoMultipleSpaceBlockquote => "MD027: multiple spaces after the blockquote symbol.",
            RuleId::NoBlanksBlockquote => "MD028: blank line inside a blockquote.",
            RuleId::SingleTrailingNewline => "MD047: files should end with exactly one trailing newline.",
            RuleId::FencedCodeLanguage => "MD040: fenced code blocks should have a language specified.",
            RuleId::CodeBlockStyle => "MD046: code block style should be consistent (fenced vs. indented).",
            RuleId::CodeFenceStyle => "MD048: code fence style should be consistent (backtick vs. tilde).",
            RuleId::BlanksAroundFences => "MD031: fenced code blocks should be surrounded by blank lines.",
            RuleId::NoBareUrls => "MD034: bare URL used without angle brackets or link syntax.",
            RuleId::NoReversedLinks => "MD011: reversed link syntax, e.g. `(text)[url]` instead of `[text](url)`.",
            RuleId::NoEmptyLinks => "MD042: links with no destination or only a placeholder like `#`.",
            RuleId::ReferenceLinksImages => "MD052: a reference link/image uses a label with no matching definition.",
            RuleId::LinkImageReferenceDefinitions => "MD053: a link/image reference definition is never used.",
            RuleId::LinkImageStyle => {
                "MD054: link/image style should be consistent (inline, reference, autolink, ...)."
            }
            RuleId::LinkFragments => "MD051: a link fragment (`#section`) doesn't match any heading in the document.",
            RuleId::RelativeLinkExists => "MD057: a relative link doesn't point to a file that exists on disk.",
            RuleId::DescriptiveLinkText => r#"MD059: link text should be descriptive, not generic like "click here"."#,
            RuleId::NoSpaceInLinks => "MD039: spaces inside link text brackets.",
            RuleId::NoSpaceInEmphasis => "MD037: spaces inside emphasis markers.",
            RuleId::NoSpaceInCode => "MD038: spaces inside code span backticks.",
            RuleId::EmphasisStyle => "MD049: emphasis style should be consistent (`*text*` vs. `_text_`).",
            RuleId::StrongStyle => "MD050: strong style should be consistent (`**text**` vs. `__text__`).",
            RuleId::NoInlineHtml => "MD033: inline HTML.",
            RuleId::ProperNames => "MD044: proper names should use the configured capitalization.",
            RuleId::CommandsShowOutput => "MD014: `$` shown before commands with no output shown.",
            RuleId::HrStyle => "MD035: horizontal rule style should be consistent.",
            RuleId::TablePipeStyle => "MD055: table pipe style should be consistent (leading/trailing pipes).",
            RuleId::TableColumnCount => {
                "MD056: every row in a table should have the same number of cells as the header."
            }
            RuleId::BlanksAroundTables => "MD058: tables should be surrounded by blank lines.",
            RuleId::TableColumnStyle => "MD060: table column style (padding around `|`) should be consistent.",
        }
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RuleId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RuleId::ALL
            .iter()
            .copied()
            .find(|id| id.as_str() == s)
            .ok_or_else(|| format!("unknown rule id `{s}`{}", did_you_mean_suffix(s)))
    }
}

/// Case-sensitive Levenshtein edit distance between `a` and `b`.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

/// The closest built-in [`RuleId`] to `input` by edit distance — `None` if nothing is close
/// enough to plausibly be a typo of it rather than an unrelated string. The threshold scales
/// with `input`'s length (never more than 3) so a short, unrelated input doesn't match anything.
pub(crate) fn closest_rule_id(input: &str) -> Option<RuleId> {
    let threshold = (input.chars().count() / 2).clamp(1, 3);
    RuleId::ALL
        .iter()
        .map(|id| (*id, levenshtein(input, id.as_str())))
        .min_by_key(|(_, dist)| *dist)
        .filter(|(_, dist)| *dist <= threshold)
        .map(|(id, _)| id)
}

/// Formats `closest_rule_id(input)` as a `" (did you mean `x`?)"` suffix for an error message,
/// or an empty string if nothing was close enough to suggest.
pub(crate) fn did_you_mean_suffix(input: &str) -> String {
    match closest_rule_id(input) {
        Some(id) => format!(" (did you mean `{}`?)", id.as_str()),
        None => String::new(),
    }
}

/// A diagnostic finding, carrying whatever data its rule needs to render a message.
/// Each variant corresponds to exactly one [`RuleId`].
#[derive(Debug, Clone, PartialEq)]
pub enum LintMessage {
    HeadingHierarchySkip { from: u8, to: u8 },
    ImageMissingAlt { url: String },
    MissingFrontMatterKey { key: String, front_matter_present: bool },
    InvalidFrontMatter { reason: String },
    HeadingStyle { expected: String, found: String },
    NoMissingSpaceAtx,
    NoMultipleSpaceAtx,
    NoMissingSpaceClosedAtx,
    NoMultipleSpaceClosedAtx,
    BlanksAroundHeadings { above: bool },
    HeadingStartLeft,
    NoDuplicateHeading { text: String },
    SingleH1,
    NoTrailingPunctuationHeading { punctuation: char },
    NoEmphasisAsHeading { text: String },
    FirstLineHeading,
    RequiredHeadings { expected: String, found: String },
    UlStyle { expected: char, found: char },
    ListIndent { expected: usize, found: usize },
    UlIndent { expected: usize, found: usize },
    OlPrefix { expected: String, found: String },
    ListMarkerSpace { expected: usize, found: usize },
    BlanksAroundLists { above: bool },
    NoTrailingSpaces,
    NoHardTabs,
    NoMultipleBlanks { count: usize },
    LineLength { length: usize, limit: usize },
    NoMultipleSpaceBlockquote,
    NoBlanksBlockquote,
    SingleTrailingNewline,
    FencedCodeLanguage,
    CodeBlockStyle { expected: String },
    CodeFenceStyle { expected: char },
    BlanksAroundFences { above: bool },
    NoBareUrls { url: String },
    NoReversedLinks { text: String },
    NoEmptyLinks,
    ReferenceLinksImages { label: String },
    LinkImageReferenceDefinitions { label: String },
    LinkImageStyle { expected: String, found: String },
    LinkFragments { fragment: String },
    RelativeLinkExists { path: String },
    DescriptiveLinkText { text: String },
    NoSpaceInLinks,
    NoSpaceInEmphasis,
    NoSpaceInCode,
    EmphasisStyle { expected: char, found: char },
    StrongStyle { expected: String, found: String },
    NoInlineHtml { tag: String },
    ProperNames { found: String, expected: String },
    CommandsShowOutput,
    HrStyle { expected: String, found: String },
    TablePipeStyle { expected: String },
    TableColumnCount { expected: usize, found: usize },
    BlanksAroundTables { above: bool },
    TableColumnStyle { expected: String },
}

impl LintMessage {
    /// The rule that produces this message.
    pub fn rule_id(&self) -> RuleId {
        match self {
            LintMessage::HeadingHierarchySkip { .. } => RuleId::HeadingHierarchySkip,
            LintMessage::ImageMissingAlt { .. } => RuleId::ImageMissingAlt,
            LintMessage::MissingFrontMatterKey { .. } | LintMessage::InvalidFrontMatter { .. } => {
                RuleId::MissingFrontMatterKey
            }
            LintMessage::HeadingStyle { .. } => RuleId::HeadingStyle,
            LintMessage::NoMissingSpaceAtx => RuleId::NoMissingSpaceAtx,
            LintMessage::NoMultipleSpaceAtx => RuleId::NoMultipleSpaceAtx,
            LintMessage::NoMissingSpaceClosedAtx => RuleId::NoMissingSpaceClosedAtx,
            LintMessage::NoMultipleSpaceClosedAtx => RuleId::NoMultipleSpaceClosedAtx,
            LintMessage::BlanksAroundHeadings { .. } => RuleId::BlanksAroundHeadings,
            LintMessage::HeadingStartLeft => RuleId::HeadingStartLeft,
            LintMessage::NoDuplicateHeading { .. } => RuleId::NoDuplicateHeading,
            LintMessage::SingleH1 => RuleId::SingleH1,
            LintMessage::NoTrailingPunctuationHeading { .. } => RuleId::NoTrailingPunctuationHeading,
            LintMessage::NoEmphasisAsHeading { .. } => RuleId::NoEmphasisAsHeading,
            LintMessage::FirstLineHeading => RuleId::FirstLineHeading,
            LintMessage::RequiredHeadings { .. } => RuleId::RequiredHeadings,
            LintMessage::UlStyle { .. } => RuleId::UlStyle,
            LintMessage::ListIndent { .. } => RuleId::ListIndent,
            LintMessage::UlIndent { .. } => RuleId::UlIndent,
            LintMessage::OlPrefix { .. } => RuleId::OlPrefix,
            LintMessage::ListMarkerSpace { .. } => RuleId::ListMarkerSpace,
            LintMessage::BlanksAroundLists { .. } => RuleId::BlanksAroundLists,
            LintMessage::NoTrailingSpaces => RuleId::NoTrailingSpaces,
            LintMessage::NoHardTabs => RuleId::NoHardTabs,
            LintMessage::NoMultipleBlanks { .. } => RuleId::NoMultipleBlanks,
            LintMessage::LineLength { .. } => RuleId::LineLength,
            LintMessage::NoMultipleSpaceBlockquote => RuleId::NoMultipleSpaceBlockquote,
            LintMessage::NoBlanksBlockquote => RuleId::NoBlanksBlockquote,
            LintMessage::SingleTrailingNewline => RuleId::SingleTrailingNewline,
            LintMessage::FencedCodeLanguage => RuleId::FencedCodeLanguage,
            LintMessage::CodeBlockStyle { .. } => RuleId::CodeBlockStyle,
            LintMessage::CodeFenceStyle { .. } => RuleId::CodeFenceStyle,
            LintMessage::BlanksAroundFences { .. } => RuleId::BlanksAroundFences,
            LintMessage::NoBareUrls { .. } => RuleId::NoBareUrls,
            LintMessage::NoReversedLinks { .. } => RuleId::NoReversedLinks,
            LintMessage::NoEmptyLinks => RuleId::NoEmptyLinks,
            LintMessage::ReferenceLinksImages { .. } => RuleId::ReferenceLinksImages,
            LintMessage::LinkImageReferenceDefinitions { .. } => RuleId::LinkImageReferenceDefinitions,
            LintMessage::LinkImageStyle { .. } => RuleId::LinkImageStyle,
            LintMessage::LinkFragments { .. } => RuleId::LinkFragments,
            LintMessage::RelativeLinkExists { .. } => RuleId::RelativeLinkExists,
            LintMessage::DescriptiveLinkText { .. } => RuleId::DescriptiveLinkText,
            LintMessage::NoSpaceInLinks => RuleId::NoSpaceInLinks,
            LintMessage::NoSpaceInEmphasis => RuleId::NoSpaceInEmphasis,
            LintMessage::NoSpaceInCode => RuleId::NoSpaceInCode,
            LintMessage::EmphasisStyle { .. } => RuleId::EmphasisStyle,
            LintMessage::StrongStyle { .. } => RuleId::StrongStyle,
            LintMessage::NoInlineHtml { .. } => RuleId::NoInlineHtml,
            LintMessage::ProperNames { .. } => RuleId::ProperNames,
            LintMessage::CommandsShowOutput => RuleId::CommandsShowOutput,
            LintMessage::HrStyle { .. } => RuleId::HrStyle,
            LintMessage::TablePipeStyle { .. } => RuleId::TablePipeStyle,
            LintMessage::TableColumnCount { .. } => RuleId::TableColumnCount,
            LintMessage::BlanksAroundTables { .. } => RuleId::BlanksAroundTables,
            LintMessage::TableColumnStyle { .. } => RuleId::TableColumnStyle,
        }
    }

    /// Suggested action for a human reviewer. Rules with a machine-applicable [`crate::Fix`]
    /// still populate this — it's shown alongside the fix, not instead of it.
    pub fn help(&self) -> Option<String> {
        match self {
            LintMessage::HeadingHierarchySkip { from, to: _ } => Some(format!(
                "insert an intermediate h{} (or renumber this heading to h{})",
                from + 1,
                from + 1
            )),
            LintMessage::ImageMissingAlt { .. } => {
                Some("describe the image's content or purpose in the alt text".to_string())
            }
            LintMessage::MissingFrontMatterKey {
                key,
                front_matter_present,
            } => Some(if *front_matter_present {
                format!("add `{key}: ...` to the front matter")
            } else {
                format!("add a front matter block with `{key}: ...`")
            }),
            LintMessage::InvalidFrontMatter { .. } => {
                Some("fix the front matter syntax so required keys can be checked".to_string())
            }
            LintMessage::HeadingStyle { expected, .. } => Some(format!("rewrite this heading in {expected} style")),
            LintMessage::NoMissingSpaceAtx => Some("add a space after the `#`".to_string()),
            LintMessage::NoMultipleSpaceAtx => Some("collapse the spaces after `#` to one".to_string()),
            LintMessage::NoMissingSpaceClosedAtx => Some("add a space before the closing `#`".to_string()),
            LintMessage::NoMultipleSpaceClosedAtx => {
                Some("collapse the spaces before the closing `#` to one".to_string())
            }
            LintMessage::BlanksAroundHeadings { above } => Some(if *above {
                "add a blank line before this heading".to_string()
            } else {
                "add a blank line after this heading".to_string()
            }),
            LintMessage::HeadingStartLeft => Some("remove the leading whitespace before this heading".to_string()),
            LintMessage::NoDuplicateHeading { .. } => Some("give this heading distinct text".to_string()),
            LintMessage::SingleH1 => Some("demote this heading, or remove the document's other h1".to_string()),
            LintMessage::NoTrailingPunctuationHeading { .. } => Some("remove the trailing punctuation".to_string()),
            LintMessage::NoEmphasisAsHeading { .. } => Some("use a real heading (`#`) instead of emphasis".to_string()),
            LintMessage::FirstLineHeading => Some("start the document with a top-level (`#`) heading".to_string()),
            LintMessage::RequiredHeadings { expected, .. } => {
                Some(format!("match the required heading structure: {expected}"))
            }
            LintMessage::UlStyle { expected, .. } => Some(format!("use `{expected}` as the list marker")),
            LintMessage::ListIndent { expected, .. } => Some(format!("indent this list item {expected} spaces")),
            LintMessage::UlIndent { expected, .. } => Some(format!("indent this list item {expected} spaces")),
            LintMessage::OlPrefix { expected, .. } => Some(format!("renumber to `{expected}`")),
            LintMessage::ListMarkerSpace { expected, .. } => {
                Some(format!("use {expected} space(s) after the list marker"))
            }
            LintMessage::BlanksAroundLists { above } => Some(if *above {
                "add a blank line before this list".to_string()
            } else {
                "add a blank line after this list".to_string()
            }),
            LintMessage::NoTrailingSpaces => Some("remove the trailing whitespace".to_string()),
            LintMessage::NoHardTabs => Some("replace the tab with spaces".to_string()),
            LintMessage::NoMultipleBlanks { .. } => Some("collapse the blank lines to one".to_string()),
            LintMessage::LineLength { limit, .. } => Some(format!("wrap this line to {limit} characters or fewer")),
            LintMessage::NoMultipleSpaceBlockquote => Some("collapse the spaces after `>` to one".to_string()),
            LintMessage::NoBlanksBlockquote => Some("remove the blank line, or split into two blockquotes".to_string()),
            LintMessage::SingleTrailingNewline => Some("end the file with exactly one newline".to_string()),
            LintMessage::FencedCodeLanguage => {
                Some("add a language after the opening fence, e.g. ` ```rust`".to_string())
            }
            LintMessage::CodeBlockStyle { expected } => Some(format!("rewrite this code block in {expected} style")),
            LintMessage::CodeFenceStyle { expected } => Some(format!("use `{expected}` as the fence character")),
            LintMessage::BlanksAroundFences { above } => Some(if *above {
                "add a blank line before this code block".to_string()
            } else {
                "add a blank line after this code block".to_string()
            }),
            LintMessage::NoBareUrls { .. } => Some("wrap the URL in `<...>` or use `[text](url)` syntax".to_string()),
            LintMessage::NoReversedLinks { .. } => Some("swap to `[text](url)` syntax".to_string()),
            LintMessage::NoEmptyLinks => Some("add a real destination, or remove the link".to_string()),
            LintMessage::ReferenceLinksImages { label } => {
                Some(format!("add a `[{label}]: url` definition, or fix the label"))
            }
            LintMessage::LinkImageReferenceDefinitions { label } => Some(format!(
                "remove the unused `[{label}]: ...` definition, or reference it"
            )),
            LintMessage::LinkImageStyle { expected, .. } => Some(format!("use {expected} style for this link/image")),
            LintMessage::LinkFragments { .. } => Some("fix the fragment, or add a matching heading".to_string()),
            LintMessage::RelativeLinkExists { .. } => Some("fix the path, or create the missing file".to_string()),
            LintMessage::DescriptiveLinkText { .. } => {
                Some("replace the link text with something that describes the destination".to_string())
            }
            LintMessage::NoSpaceInLinks => Some("remove the spaces just inside `[...]`".to_string()),
            LintMessage::NoSpaceInEmphasis => Some("remove the spaces just inside the emphasis markers".to_string()),
            LintMessage::NoSpaceInCode => Some("remove the spaces just inside the backticks".to_string()),
            LintMessage::EmphasisStyle { expected, .. } => Some(format!("use `{expected}` for emphasis")),
            LintMessage::StrongStyle { expected, .. } => Some(format!("use `{expected}` for strong text")),
            LintMessage::NoInlineHtml { .. } => Some("use Markdown syntax instead of raw HTML".to_string()),
            LintMessage::ProperNames { expected, .. } => Some(format!("use the capitalization `{expected}`")),
            LintMessage::CommandsShowOutput => {
                Some("remove the `$ ` prefixes, or show the command's output below it".to_string())
            }
            LintMessage::HrStyle { expected, .. } => Some(format!("use `{expected}` for horizontal rules")),
            LintMessage::TablePipeStyle { expected } => Some(format!("use {expected} pipe style for this table")),
            LintMessage::TableColumnCount { expected, .. } => {
                Some(format!("add or remove cells so this row has {expected} column(s)"))
            }
            LintMessage::BlanksAroundTables { above } => Some(if *above {
                "add a blank line before this table".to_string()
            } else {
                "add a blank line after this table".to_string()
            }),
            LintMessage::TableColumnStyle { expected } => Some(format!("use {expected} column style for this table")),
        }
    }
}

impl fmt::Display for LintMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LintMessage::HeadingHierarchySkip { from, to } => {
                write!(f, "heading level jumps from h{from} to h{to}, skipping a level")
            }
            LintMessage::ImageMissingAlt { url } => write!(f, "image `{url}` has no alt text"),
            LintMessage::MissingFrontMatterKey {
                key,
                front_matter_present,
            } => {
                if *front_matter_present {
                    write!(f, "front matter is missing required key `{key}`")
                } else {
                    write!(
                        f,
                        "document has no front matter block (required key `{key}` is missing)"
                    )
                }
            }
            LintMessage::InvalidFrontMatter { reason } => write!(f, "front matter could not be parsed: {reason}"),
            LintMessage::HeadingStyle { expected, found } => {
                write!(
                    f,
                    "heading style is {found}, expected {expected} (for consistency with earlier headings)"
                )
            }
            LintMessage::NoMissingSpaceAtx => write!(f, "no space after `#` on this ATX heading"),
            LintMessage::NoMultipleSpaceAtx => write!(f, "multiple spaces after `#` on this ATX heading"),
            LintMessage::NoMissingSpaceClosedAtx => write!(f, "no space before the closing `#` on this heading"),
            LintMessage::NoMultipleSpaceClosedAtx => {
                write!(f, "multiple spaces before the closing `#` on this heading")
            }
            LintMessage::BlanksAroundHeadings { above } => {
                write!(
                    f,
                    "heading is missing a blank line {}",
                    if *above { "before it" } else { "after it" }
                )
            }
            LintMessage::HeadingStartLeft => write!(f, "heading does not start at the beginning of the line"),
            LintMessage::NoDuplicateHeading { text } => write!(f, "heading `{text}` duplicates an earlier heading"),
            LintMessage::SingleH1 => write!(f, "multiple top-level (h1) headings in this document"),
            LintMessage::NoTrailingPunctuationHeading { punctuation } => {
                write!(f, "heading ends with trailing punctuation `{punctuation}`")
            }
            LintMessage::NoEmphasisAsHeading { text } => {
                write!(f, "emphasized line `{text}` looks like it's meant to be a heading")
            }
            LintMessage::FirstLineHeading => write!(f, "document does not start with a top-level heading"),
            LintMessage::RequiredHeadings { expected, found } => {
                write!(
                    f,
                    "heading structure does not match required structure: expected {expected}, found {found}"
                )
            }
            LintMessage::UlStyle { expected, found } => {
                write!(
                    f,
                    "unordered list marker `{found}`, expected `{expected}` (for consistency)"
                )
            }
            LintMessage::ListIndent { expected, found } => {
                write!(f, "list item indented {found} spaces, expected {expected}")
            }
            LintMessage::UlIndent { expected, found } => {
                write!(f, "unordered list item indented {found} spaces, expected {expected}")
            }
            LintMessage::OlPrefix { expected, found } => {
                write!(f, "ordered list item prefix `{found}`, expected `{expected}`")
            }
            LintMessage::ListMarkerSpace { expected, found } => {
                write!(f, "{found} space(s) after the list marker, expected {expected}")
            }
            LintMessage::BlanksAroundLists { above } => {
                write!(
                    f,
                    "list is missing a blank line {}",
                    if *above { "before it" } else { "after it" }
                )
            }
            LintMessage::NoTrailingSpaces => write!(f, "line has trailing whitespace"),
            LintMessage::NoHardTabs => write!(f, "line contains a hard tab"),
            LintMessage::NoMultipleBlanks { count } => write!(f, "{count} consecutive blank lines"),
            LintMessage::LineLength { length, limit } => {
                write!(f, "line is {length} characters long, limit is {limit}")
            }
            LintMessage::NoMultipleSpaceBlockquote => write!(f, "multiple spaces after the blockquote `>`"),
            LintMessage::NoBlanksBlockquote => write!(f, "blank line inside a blockquote"),
            LintMessage::SingleTrailingNewline => write!(f, "file does not end with exactly one newline"),
            LintMessage::FencedCodeLanguage => write!(f, "fenced code block has no language specified"),
            LintMessage::CodeBlockStyle { expected } => {
                write!(f, "code block style should be {expected} (for consistency)")
            }
            LintMessage::CodeFenceStyle { expected } => {
                write!(f, "code fence character should be `{expected}` (for consistency)")
            }
            LintMessage::BlanksAroundFences { above } => {
                write!(
                    f,
                    "code block is missing a blank line {}",
                    if *above { "before it" } else { "after it" }
                )
            }
            LintMessage::NoBareUrls { url } => write!(f, "bare URL `{url}` used without link syntax"),
            LintMessage::NoReversedLinks { text } => write!(f, "reversed link syntax `{text}`"),
            LintMessage::NoEmptyLinks => write!(f, "link has no real destination"),
            LintMessage::ReferenceLinksImages { label } => {
                write!(f, "reference `[{label}]` has no matching `[{label}]: url` definition")
            }
            LintMessage::LinkImageReferenceDefinitions { label } => {
                write!(f, "reference definition `[{label}]` is never used")
            }
            LintMessage::LinkImageStyle { expected, found } => {
                write!(f, "link/image style is {found}, expected {expected} (for consistency)")
            }
            LintMessage::LinkFragments { fragment } => {
                write!(
                    f,
                    "link fragment `#{fragment}` does not match any heading in this document"
                )
            }
            LintMessage::RelativeLinkExists { path } => {
                write!(f, "relative link `{path}` does not point to an existing file")
            }
            LintMessage::DescriptiveLinkText { text } => write!(f, "link text `{text}` is not descriptive"),
            LintMessage::NoSpaceInLinks => write!(f, "space just inside the link text brackets"),
            LintMessage::NoSpaceInEmphasis => write!(f, "space just inside the emphasis markers"),
            LintMessage::NoSpaceInCode => write!(f, "space just inside the code span backticks"),
            LintMessage::EmphasisStyle { expected, found } => {
                write!(f, "emphasis marker `{found}`, expected `{expected}` (for consistency)")
            }
            LintMessage::StrongStyle { expected, found } => {
                write!(f, "strong marker `{found}`, expected `{expected}` (for consistency)")
            }
            LintMessage::NoInlineHtml { tag } => write!(f, "inline HTML element `<{tag}>`"),
            LintMessage::ProperNames { found, expected } => {
                write!(f, "`{found}` should be capitalized as `{expected}`")
            }
            LintMessage::CommandsShowOutput => write!(f, "`$ ` command prefix used with no output shown"),
            LintMessage::HrStyle { expected, found } => {
                write!(f, "horizontal rule `{found}`, expected `{expected}` (for consistency)")
            }
            LintMessage::TablePipeStyle { expected } => write!(f, "table row pipe style should be {expected}"),
            LintMessage::TableColumnCount { expected, found } => {
                write!(
                    f,
                    "table row has {found} column(s), expected {expected} (matching the header)"
                )
            }
            LintMessage::BlanksAroundTables { above } => {
                write!(
                    f,
                    "table is missing a blank line {}",
                    if *above { "before it" } else { "after it" }
                )
            }
            LintMessage::TableColumnStyle { expected } => {
                write!(f, "table column style should be {expected}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_id_round_trips_through_str() {
        for id in RuleId::ALL {
            assert_eq!(id.as_str().parse::<RuleId>().unwrap(), *id);
        }
    }

    #[test]
    fn rule_id_from_str_rejects_unknown() {
        assert!("not_a_real_rule".parse::<RuleId>().is_err());
    }

    #[test]
    fn rule_id_from_str_suggests_a_close_typo() {
        let err = "line_lenght".parse::<RuleId>().unwrap_err();
        assert!(
            err.contains("did you mean `line_length`?"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn rule_id_from_str_does_not_suggest_for_an_unrelated_string() {
        let err = "xyzabc123".parse::<RuleId>().unwrap_err();
        assert!(!err.contains("did you mean"), "unexpected error message: {err}");
    }

    #[test]
    fn closest_rule_id_finds_every_rule_ids_own_typo_of_itself() {
        // A one-character truncation of every rule id should still resolve back to it — a cheap
        // way to exercise the threshold against the full, varied-length rule id list rather than
        // just a couple of hand-picked examples.
        for id in RuleId::ALL {
            let name = id.as_str();
            let truncated = &name[..name.len() - 1];
            assert_eq!(
                closest_rule_id(truncated),
                Some(*id),
                "truncating {name} to {truncated} should still suggest {name}"
            );
        }
    }

    #[test]
    fn all_rule_ids_are_unique() {
        let mut ids: Vec<&str> = RuleId::ALL.iter().map(RuleId::as_str).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate rule id string");
    }

    #[test]
    fn every_rule_id_has_a_non_empty_description() {
        for id in RuleId::ALL {
            assert!(!id.description().is_empty(), "{id} has no description");
        }
    }

    #[test]
    fn message_rule_id_matches_intent() {
        let msg = LintMessage::HeadingHierarchySkip { from: 1, to: 3 };
        assert_eq!(msg.rule_id(), RuleId::HeadingHierarchySkip);
        assert_eq!(msg.to_string(), "heading level jumps from h1 to h3, skipping a level");
        assert!(msg.help().unwrap().contains("h2"));
    }
}
