//! MD060: table column style — the padding around pipe characters (`|`) in a GFM table's cells
//! should be consistent. `[rules.table_column_style]`:
//! - `style` (default `"any"`): `"compact"` (exactly one space of padding on each side of a
//!   cell's content — an empty cell is just one space, `| |`, not two), `"tight"` (no padding at
//!   all), `"aligned"` (every header/body row's pipes
//!   land at the same character column, so the table lines up visually — padding amount itself
//!   isn't checked), or `"any"` — whichever of the three needs the fewest changes across the
//!   document, the same "pick whichever fits" default [`super::table_pipe_style`] uses.
//! - `aligned_delimiter` (default `false`): when the resolved style is `"aligned"`, also require
//!   the delimiter row's (`---`) pipes to line up with the header/body rows'. Off by default,
//!   and the delimiter row is never checked under `"compact"`/`"tight"` regardless of this
//!   setting — a `---` delimiter is conventionally written with no padding even in an otherwise
//!   padded table (`| A | B |` / `|---|---|` / `| 1 | 2 |` is normal, expected `"compact"`
//!   formatting, not a violation), so treating it like an ordinary row would flag the vast
//!   majority of real-world tables.
//!
//! Scoped to rows with both a leading and trailing pipe (`| a | b |`, not `a | b`) — the
//! overwhelmingly common GFM table style, and what [`super::table_pipe_style`] normalizes toward
//! anyway; a row without one is left unchecked here rather than guessed at. Cell width is
//! measured in `char`s, not display width, so a table with CJK or emoji content may be flagged
//! (or not) based on character count even though it renders aligned (or misaligned) in a
//! renderer that draws those characters double-width — unlike markdownlint's own MD060, which
//! accounts for this; narrow enough in practice that this crate doesn't pull in a width
//! calculation dependency for it. Only `"compact"`/`"tight"` violations are auto-fixable —
//! realigning `"aligned"` isn't a single-row rewrite (matches markdownlint's own MD060, which
//! doesn't autofix `aligned` either).

use std::collections::HashSet;

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, Fix, LintConfig, LintMessage, Range, RuleId, Severity};

pub(crate) struct TableColumnStyle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Compact,
    Tight,
    Aligned,
}

impl Style {
    fn as_str(self) -> &'static str {
        match self {
            Style::Compact => "compact",
            Style::Tight => "tight",
            Style::Aligned => "aligned",
        }
    }
}

/// One table row, in document order: its 1-based line number and whether it's the delimiter
/// (`---`) row.
struct Row {
    line: usize,
    is_delimiter: bool,
}

/// Splits a row's raw line into its pipe-delimited cell segments (untrimmed, so callers can
/// inspect padding) — `None` if the line doesn't have both a leading and trailing `|` (out of
/// scope; see module docs).
fn row_cells(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    Some(trimmed[1..trimmed.len() - 1].split('|').collect())
}

/// Whether `segment` (a cell's raw text between two pipes) has exactly one space of padding on
/// each side of its content — or, for an empty cell, is exactly one space (`| |`, the near-
/// universal way people hand-write an empty cell — a leading and trailing space each would
/// double-count as two for an empty cell with nothing between them to separate, which is correct
/// in principle but flags essentially every empty cell anyone has ever written).
fn is_compact_cell(segment: &str) -> bool {
    if segment.trim().is_empty() {
        return segment.chars().count() == 1;
    }
    let leading = segment.chars().take_while(|c| c.is_whitespace()).count();
    let trailing = segment.chars().rev().take_while(|c| c.is_whitespace()).count();
    leading == 1 && trailing == 1
}

/// Whether `segment` has no padding at all around its content (or is empty, for an empty cell).
fn is_tight_cell(segment: &str) -> bool {
    if segment.trim().is_empty() {
        return segment.is_empty();
    }
    let leading = segment.chars().take_while(|c| c.is_whitespace()).count();
    let trailing = segment.chars().rev().take_while(|c| c.is_whitespace()).count();
    leading == 0 && trailing == 0
}

fn rewrite_cell(segment: &str, style: Style) -> String {
    let content = segment.trim();
    match style {
        Style::Compact if content.is_empty() => " ".to_string(),
        Style::Compact => format!(" {content} "),
        Style::Tight => content.to_string(),
        Style::Aligned => segment.to_string(),
    }
}

/// Rewrites every cell in a row (given its already-split segments) to `style`, reassembling the
/// full `| ... | ... |` line.
fn rewrite_row(cells: &[&str], style: Style) -> String {
    let inner = cells
        .iter()
        .map(|c| rewrite_cell(c, style))
        .collect::<Vec<_>>()
        .join("|");
    format!("|{inner}|")
}

/// Groups a document's table rows into per-table blocks — a document can contain more than one
/// table, and each is checked independently (an "aligned" table's column widths have nothing to
/// do with a different table's).
fn tables(doc: &mq_markdown::Markdown) -> Vec<Vec<Row>> {
    let mut tables = Vec::new();
    let mut current: Vec<Row> = Vec::new();
    let mut seen = HashSet::new();

    for node in &doc.nodes {
        match node {
            Node::TableCell(cell) => {
                if let Some(position) = &cell.position
                    && seen.insert(position.start.line)
                {
                    current.push(Row {
                        line: position.start.line,
                        is_delimiter: false,
                    });
                }
            }
            Node::TableAlign(align) => {
                if let Some(position) = &align.position
                    && seen.insert(position.start.line)
                {
                    current.push(Row {
                        line: position.start.line,
                        is_delimiter: true,
                    });
                }
            }
            _ if !current.is_empty() => {
                tables.push(std::mem::take(&mut current));
                seen.clear();
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        tables.push(current);
    }
    tables
}

/// A table row eligible for this rule (has a leading and trailing pipe), with its already-split
/// cell segments.
struct EligibleRow<'a> {
    line: usize,
    is_delimiter: bool,
    cells: Vec<&'a str>,
}

/// Resolves `table`'s rows to raw text via `lines`, keeping only rows [`row_cells`] can parse and
/// whose column count matches the table's first row (a count mismatch is `table_column_count`'s
/// concern, not this rule's — comparing padding/alignment across columns that don't correspond
/// to each other would just be noise).
fn eligible_rows<'a>(table: &[Row], lines: &crate::text::LineIndex<'a>) -> Vec<EligibleRow<'a>> {
    let mut rows = Vec::new();
    let mut expected_columns = None;
    for row in table {
        let Some(line) = lines.get(row.line) else { continue };
        let Some(cells) = row_cells(line) else { continue };
        let expected = *expected_columns.get_or_insert(cells.len());
        if cells.len() != expected {
            continue;
        }
        rows.push(EligibleRow {
            line: row.line,
            is_delimiter: row.is_delimiter,
            cells,
        });
    }
    rows
}

/// Rows a violation check applies to for `style` — every eligible row for `"compact"`/`"tight"`
/// except the delimiter row (see module docs), or every eligible row for `"aligned"` (the
/// delimiter row is separately gated by `aligned_delimiter` in [`aligned_violations`]).
fn checkable_rows<'a, 'b>(rows: &'b [EligibleRow<'a>], style: Style) -> Vec<&'b EligibleRow<'a>> {
    rows.iter()
        .filter(|row| style == Style::Aligned || !row.is_delimiter)
        .collect()
}

fn compact_violations<'a>(rows: &[&EligibleRow<'a>]) -> Vec<usize> {
    rows.iter()
        .filter(|row| !row.cells.iter().all(|cell| is_compact_cell(cell)))
        .map(|row| row.line)
        .collect()
}

fn tight_violations<'a>(rows: &[&EligibleRow<'a>]) -> Vec<usize> {
    rows.iter()
        .filter(|row| !row.cells.iter().all(|cell| is_tight_cell(cell)))
        .map(|row| row.line)
        .collect()
}

/// A row is aligned-violating if any of its cell segments' `char` length differs from the first
/// (non-delimiter, unless `aligned_delimiter`) row's — that's what makes every row's pipes land
/// on the same character column.
fn aligned_violations(rows: &[&EligibleRow], aligned_delimiter: bool) -> Vec<usize> {
    let reference = rows.iter().find(|row| aligned_delimiter || !row.is_delimiter);
    let Some(reference) = reference else { return Vec::new() };
    let widths: Vec<usize> = reference.cells.iter().map(|c| c.chars().count()).collect();

    rows.iter()
        .filter(|row| aligned_delimiter || !row.is_delimiter)
        .filter(|row| {
            row.cells.len() != widths.len()
                || row
                    .cells
                    .iter()
                    .zip(&widths)
                    .any(|(cell, width)| cell.chars().count() != *width)
        })
        .map(|row| row.line)
        .collect()
}

impl Rule for TableColumnStyle {
    fn id(&self) -> RuleId {
        RuleId::TableColumnStyle
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        source: &str,
        config: &LintConfig,
        _path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let options = config.rule_options(self.id());
        let configured_style = options.get_str("style").and_then(|s| match s {
            "compact" => Some(Style::Compact),
            "tight" => Some(Style::Tight),
            "aligned" => Some(Style::Aligned),
            _ => None,
        });
        let aligned_delimiter = options.get_bool("aligned_delimiter").unwrap_or(false);

        let lines = crate::text::LineIndex::new(source);
        let per_table_rows: Vec<Vec<EligibleRow>> =
            tables(doc).iter().map(|table| eligible_rows(table, &lines)).collect();

        let style = configured_style.unwrap_or_else(|| {
            // "any": whichever style needs the fewest changes across the whole document.
            let mut counts = [
                (Style::Compact, 0usize),
                (Style::Tight, 0usize),
                (Style::Aligned, 0usize),
            ];
            for rows in &per_table_rows {
                for (candidate, count) in &mut counts {
                    let checkable = checkable_rows(rows, *candidate);
                    let violations = match candidate {
                        Style::Compact => compact_violations(&checkable),
                        Style::Tight => tight_violations(&checkable),
                        Style::Aligned => aligned_violations(&checkable, aligned_delimiter),
                    };
                    *count += violations.len();
                }
            }
            counts
                .into_iter()
                .min_by_key(|(_, count)| *count)
                .map(|(style, _)| style)
                .unwrap()
        });

        let mut diagnostics = Vec::new();
        for rows in &per_table_rows {
            let checkable = checkable_rows(rows, style);
            let violating_lines: HashSet<usize> = match style {
                Style::Compact => compact_violations(&checkable),
                Style::Tight => tight_violations(&checkable),
                Style::Aligned => aligned_violations(&checkable, aligned_delimiter),
            }
            .into_iter()
            .collect();

            for row in rows.iter().filter(|row| violating_lines.contains(&row.line)) {
                let Some(line) = lines.get(row.line) else { continue };
                let range = Range::single_line(row.line, 1, line.chars().count() + 1);
                let mut diagnostic = Diagnostic::new(
                    LintMessage::TableColumnStyle {
                        expected: style.as_str().to_string(),
                    },
                    self.default_severity(),
                )
                .with_range(range);
                if style != Style::Aligned {
                    diagnostic = diagnostic.with_fix(Fix::new(range, rewrite_row(&row.cells, style)));
                }
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }

    fn option_keys(&self) -> &'static [&'static str] {
        &["style", "aligned_delimiter"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with_config(markdown: &str, config_toml: &str) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        let config = LintConfig::from_toml_str(config_toml).unwrap();
        TableColumnStyle.check(&doc, markdown, &config, None)
    }

    fn run_compact(markdown: &str) -> Vec<Diagnostic> {
        run_with_config(markdown, "[rules.table_column_style]\nstyle = \"compact\"\n")
    }

    fn run_tight(markdown: &str) -> Vec<Diagnostic> {
        run_with_config(markdown, "[rules.table_column_style]\nstyle = \"tight\"\n")
    }

    #[test]
    fn no_diagnostics_for_a_consistently_compact_table() {
        assert!(run_compact("| A | B |\n|---|---|\n| 1 | 2 |\n").is_empty());
    }

    #[test]
    fn delimiter_row_padding_is_never_checked_under_compact() {
        // A delimiter row is conventionally tight even in an otherwise compact table.
        assert!(run_compact("| A | B |\n| --- | --- |\n| 1 | 2 |\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_row_missing_compact_padding() {
        let diagnostics = run_compact("| A | B |\n|---|---|\n|1 | 2 |\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "| 1 | 2 |");
    }

    #[test]
    fn a_single_space_empty_cell_is_compact_not_two_spaces() {
        // The near-universal way people hand-write an empty cell — must not be flagged.
        assert!(run_compact("| A | B |\n|---|---|\n| 1 | |\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_two_space_empty_cell_under_compact() {
        let diagnostics = run_compact("| A | B |\n|---|---|\n| 1 |  |\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "| 1 | |");
    }

    #[test]
    fn no_diagnostics_for_a_consistently_tight_table() {
        assert!(run_tight("|A|B|\n|---|---|\n|1|2|\n").is_empty());
    }

    #[test]
    fn flags_and_fixes_a_row_with_extra_tight_padding() {
        let diagnostics = run_tight("|A|B|\n|---|---|\n|1 |2|\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].fix.as_ref().unwrap().replacement, "|1|2|");
    }

    #[test]
    fn no_diagnostics_for_an_aligned_table_with_ragged_delimiter() {
        let diagnostics = run_with_config(
            "| A   | B |\n|-----|---|\n| 1   | 2 |\n",
            "[rules.table_column_style]\nstyle = \"aligned\"\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_an_unaligned_row_without_a_fix() {
        let diagnostics = run_with_config(
            "| A   | B |\n|-----|---|\n| 1 | 2 |\n",
            "[rules.table_column_style]\nstyle = \"aligned\"\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].fix.is_none());
    }

    #[test]
    fn aligned_delimiter_option_also_checks_the_delimiter_row() {
        // The delimiter row's segments ("---", "---" — 3 chars each) don't match the header's
        // (" A   " is 5 chars) — a violation only once aligned_delimiter opts the row in.
        let markdown = "| A   | B |\n|---|---|\n| 1   | 2 |\n";
        assert!(run_with_config(markdown, "[rules.table_column_style]\nstyle = \"aligned\"\n").is_empty());
        let diagnostics = run_with_config(
            markdown,
            "[rules.table_column_style]\nstyle = \"aligned\"\naligned_delimiter = true\n",
        );
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn any_style_picks_whichever_needs_the_fewest_changes() {
        // Every row here is already tight; compact/aligned would need changes to every row.
        let diagnostics = run_with_config("|A|B|\n|---|---|\n|1|2|\n", "");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn rows_without_a_leading_and_trailing_pipe_are_left_unchecked() {
        assert!(run_compact("A | B\n---|---\n1 | 2\n").is_empty());
    }

    #[test]
    fn each_table_in_a_document_is_checked_independently() {
        let diagnostics = run_compact("| A | B |\n|---|---|\n| 1 | 2 |\n\ntext\n\n|C|D|\n|---|---|\n|3|4|\n");
        // Only the second (tight) table's rows should be flagged under a compact expectation.
        assert_eq!(diagnostics.len(), 2);
    }
}
