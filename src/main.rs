mod format;
mod watch;

use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::{CommandFactory, Parser};
use colored::Colorize;
use format::OutputFormat;
use mq_content_lint::report_item::ReportItem;
use mq_content_lint::{LintConfig, Linter, RuleId, Severity};
use rayon::prelude::*;

/// Static content linter for Markdown, built on mq's AST and selectors.
#[derive(Parser)]
#[command(name = "mq-content-lint", about = "Lint Markdown content", version)]
struct Cli {
    /// Markdown files or directories to lint (reads from stdin if omitted). Directories are
    /// searched recursively for `.md`/`.markdown` files, skipping dotfiles/dotdirs.
    files: Vec<PathBuf>,

    /// Path to a `mq-content-lint.toml` config file. When omitted, the current directory (and
    /// its ancestors) are searched for one.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Disable a rule by ID (repeatable). Always wins over the config file.
    #[arg(long = "disable", value_name = "RULE_ID")]
    disable: Vec<RuleId>,

    /// Only report diagnostics at or above this severity (info, warning, error)
    #[arg(long, default_value = "info")]
    min_severity: SeverityArg,

    /// Print all built-in rule IDs, their default severity, and mq selector, then exit
    #[arg(long)]
    list_rules: bool,

    /// Print a built-in rule's description, markdownlint equivalent (if any), default severity,
    /// mq selector, and configurable options, then exit
    #[arg(long, value_name = "RULE_ID")]
    explain: Option<RuleId>,

    /// Print a shell completion script for the given shell to stdout, then exit. Install it the
    /// way that shell expects, e.g. for bash:
    /// `mq-content-lint --generate-completions bash > /etc/bash_completion.d/mq-content-lint`.
    #[arg(long, value_name = "SHELL")]
    generate_completions: Option<clap_complete::Shell>,

    /// Print a roff man page for this CLI to stdout, then exit, e.g.
    /// `mq-content-lint --generate-man-page > /usr/local/share/man/man1/mq-content-lint.1`.
    #[arg(long)]
    generate_man_page: bool,

    /// Print a JSON Schema (draft-07) for `mq-content-lint.toml` to stdout, then exit. Save it
    /// next to your config and reference it with a `#:schema ./mq-content-lint.schema.json`
    /// pragma comment (supported by Taplo / the "Even Better TOML" VS Code extension) for
    /// autocomplete and validation while editing.
    #[arg(long)]
    print_json_schema: bool,

    /// Rewrite files in place, applying every diagnostic with a machine-applicable fix (reads
    /// stdin if no files are given, writing the fixed content to stdout). Re-lints and re-fixes
    /// automatically when a fix exposes a new diagnostic, up to a bounded number of passes (the
    /// same convention ESLint's own `--fix` uses), so one run converges without a second
    /// invocation in the common case.
    #[arg(long)]
    fix: bool,

    /// Preview what --fix would change, as a unified diff, without writing anything (files stay
    /// untouched; stdin's fixed content is not printed). Implies computing fixes on its own, so
    /// it doesn't need --fix too. Exits non-zero if any file would change, like `--fix` exits
    /// non-zero on remaining diagnostics.
    #[arg(long)]
    diff: bool,

    /// Diagnostic output format
    #[arg(long, value_enum, default_value_t)]
    format: OutputFormat,

    /// Re-run after any change to a watched file (or a `.md`/`.markdown` file created under a
    /// watched directory), printing a fresh report each time instead of exiting. Combine with
    /// `--fix` to re-fix on save. Requires at least one file/directory argument — there's no
    /// sense watching stdin. Runs until interrupted (Ctrl+C).
    #[arg(long)]
    watch: bool,
}

#[derive(Clone, Copy)]
struct SeverityArg(Severity);

impl FromStr for SeverityArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(SeverityArg(Severity::Info)),
            "warning" | "warn" => Ok(SeverityArg(Severity::Warning)),
            "error" => Ok(SeverityArg(Severity::Error)),
            other => Err(format!("invalid severity `{other}` (expected info, warning, or error)")),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(had_diagnostics) => {
            if had_diagnostics {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("{} {}", "error:".bright_red().bold(), e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<bool> {
    let cli = Cli::parse();
    let mut w = BufWriter::new(io::stdout());

    if cli.list_rules {
        list_rules(&mut w)?;
        return Ok(false);
    }

    if let Some(rule_id) = cli.explain {
        explain_rule(&mut w, rule_id)?;
        return Ok(false);
    }

    if let Some(shell) = cli.generate_completions {
        generate_completions(shell, &mut w)?;
        return Ok(false);
    }

    if cli.generate_man_page {
        generate_man_page(&mut w)?;
        return Ok(false);
    }

    if cli.print_json_schema {
        print_json_schema(&mut w)?;
        return Ok(false);
    }

    let mut config = if let Some(path) = &cli.config {
        LintConfig::load_from_path(path).map_err(|e| io::Error::other(e.to_string()))?
    } else {
        let cwd = std::env::current_dir()?;
        LintConfig::discover(&cwd).map_err(|e| io::Error::other(e.to_string()))?
    };
    for rule_id in &cli.disable {
        config.disable_rule(*rule_id);
    }
    let min_severity = cli.min_severity.0;
    let linter = Linter::with_default_rules();

    if cli.files.is_empty() {
        if cli.watch {
            return Err(io::Error::other(
                "--watch requires at least one file or directory argument",
            ));
        }

        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;

        if cli.fix || cli.diff {
            let (fixed, _, _) = fix_source(&content, &linter, &config)?;
            if cli.diff {
                let changed = fixed != content;
                if changed {
                    write!(w, "{}", compute_diff("<stdin>", &content, &fixed))?;
                }
                return Ok(changed);
            }
            write!(w, "{fixed}")?;
            return Ok(false);
        }

        let diagnostics = lint_content(&content, &linter, &config, min_severity)?;
        let had_diagnostics = !diagnostics.is_empty();
        format::write_report(&mut w, cli.format, &[("<stdin>".to_string(), diagnostics)])?;
        return Ok(had_diagnostics);
    }

    if cli.watch {
        return watch::run(&cli, &config, &linter, min_severity, &mut w);
    }

    lint_files(&cli, &config, &linter, min_severity, &mut w)
}

/// Resolves `cli.files` to concrete Markdown files, lints (and optionally fixes/diffs) them in
/// parallel, and writes the report. Re-walks `cli.files` on every call rather than caching the
/// resolved list, so [`watch::run`] picks up files created or deleted between runs.
fn lint_files(
    cli: &Cli,
    config: &LintConfig,
    linter: &Linter,
    min_severity: Severity,
    w: &mut impl Write,
) -> io::Result<bool> {
    let mut paths = Vec::new();
    for arg in &cli.files {
        collect_markdown_files(arg, config, &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    // Each file's read/fix/lint pipeline is independent (only file I/O and pure computation,
    // no shared mutable state), so it parallelizes cleanly across files with rayon.
    // `par_iter().map(..).collect()` preserves `paths`' order in the output regardless of which
    // thread finishes first, so output stays deterministic; anything that writes to `w` (the
    // "fixed N issues" notices, diffs) is deferred to a sequential pass afterward for the same
    // reason.
    let outcomes: Vec<FileOutcome> = paths
        .par_iter()
        .map(|path| -> io::Result<FileOutcome> { process_file(path, linter, config, min_severity, cli.fix, cli.diff) })
        .collect::<io::Result<Vec<_>>>()?;

    if cli.diff {
        let mut any_diff = false;
        for outcome in &outcomes {
            if let Some(diff_text) = &outcome.diff {
                any_diff = true;
                write!(w, "{diff_text}")?;
            }
        }
        return Ok(any_diff);
    }

    for outcome in &outcomes {
        if let Some(fix_count) = outcome.fix_count {
            let issue_word = if fix_count == 1 { "issue" } else { "issues" };
            if cli.format == OutputFormat::Text {
                writeln!(
                    w,
                    "{} {fix_count} {issue_word} in {}",
                    "fixed".bright_green().bold(),
                    outcome.label
                )?;
            } else {
                eprintln!("fixed {fix_count} {issue_word} in {}", outcome.label);
            }
        }
    }

    let had_diagnostics = outcomes.iter().any(|o| !o.diagnostics.is_empty());
    let results: Vec<(String, Vec<ReportItem>)> = outcomes.into_iter().map(|o| (o.label, o.diagnostics)).collect();
    format::write_report(w, cli.format, &results)?;

    Ok(had_diagnostics)
}

/// One file's outcome from [`process_file`]: its diagnostics; when `--fix` changed it, how many
/// fixes were applied (for the sequential "fixed N issues in ..." notice); when `--diff` would
/// have changed it, the unified diff text (for the sequential diff printout). At most one of
/// `fix_count`/`diff` is ever set, since `--fix` and `--diff` don't write in the same run.
struct FileOutcome {
    label: String,
    diagnostics: Vec<ReportItem>,
    fix_count: Option<usize>,
    diff: Option<String>,
}

/// Reads, optionally fixes, and lints a single file. Pure I/O plus computation with no access to
/// shared state, so callers can run this across a rayon `par_iter()` safely.
///
/// `fix` rewrites the file in place when changed, reusing [`fix_source`]'s own final-pass
/// diagnostics for the report instead of linting the (already fully linted) result a second time.
/// `diff` computes the same fix but never writes — it captures a unified diff instead, and skips
/// building a diagnostic report entirely, since [`lint_files`]'s `--diff` branch never reads
/// `FileOutcome::diagnostics`. If both are set, `diff` wins (no write); `--fix --diff` is accepted
/// but behaves like `--diff` alone.
fn process_file(
    path: &Path,
    linter: &Linter,
    config: &LintConfig,
    min_severity: Severity,
    fix: bool,
    diff: bool,
) -> io::Result<FileOutcome> {
    let original = std::fs::read_to_string(path)
        .map_err(|e| io::Error::other(format!("reading file {}: {}", path.display(), e)))?;
    let label = path.display().to_string();

    if diff {
        let (fixed, _, _) = fix_source(&original, linter, config)
            .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;
        let file_diff = (fixed != original).then(|| compute_diff(&label, &original, &fixed));
        return Ok(FileOutcome {
            label,
            diagnostics: Vec::new(),
            fix_count: None,
            diff: file_diff,
        });
    }

    let (fix_count, items) = if fix {
        let (fixed, count, items) = fix_source(&original, linter, config)
            .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;
        if fixed == original {
            (None, items)
        } else {
            std::fs::write(path, &fixed)
                .map_err(|e| io::Error::other(format!("writing file {}: {}", path.display(), e)))?;
            (Some(count), items)
        }
    } else {
        let items = lint_items(&original, linter, config)
            .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;
        (None, items)
    };

    let diagnostics = items
        .into_iter()
        .filter(|item| item.severity() >= min_severity)
        .collect();

    Ok(FileOutcome {
        label,
        diagnostics,
        fix_count,
        diff: None,
    })
}

/// A unified diff between `old` and `new`, headered with `label` (`a/<label>` / `b/<label>`,
/// like `git diff`'s default headers).
fn compute_diff(label: &str, old: &str, new: &str) -> String {
    let diff = similar::TextDiff::from_lines(old, new);
    diff.unified_diff()
        .header(&format!("a/{label}"), &format!("b/{label}"))
        .to_string()
}

/// Parses `content` as Markdown and returns every built-in plus custom-rule diagnostic, merged
/// and sorted by position — unfiltered by severity, since a caller computing fixes needs every
/// diagnostic that carries one regardless of severity (severity filtering is purely a reporting
/// concern; see [`lint_content`]).
fn lint_items(content: &str, linter: &Linter, config: &LintConfig) -> io::Result<Vec<ReportItem>> {
    let doc: mq_markdown::Markdown = content
        .parse()
        .map_err(|e: miette::Error| io::Error::other(e.to_string()))?;
    mq_content_lint::report_item::lint(&doc, content, linter, config).map_err(io::Error::other)
}

/// [`lint_items`], filtered to diagnostics at or above `min_severity` — what the CLI actually
/// reports.
fn lint_content(
    content: &str,
    linter: &Linter,
    config: &LintConfig,
    min_severity: Severity,
) -> io::Result<Vec<ReportItem>> {
    Ok(lint_items(content, linter, config)?
        .into_iter()
        .filter(|item| item.severity() >= min_severity)
        .collect())
}

/// Passes `fix_source_once` re-lints and re-fixes for before giving up — fixing one diagnostic
/// occasionally exposes another (e.g. fixing a heading's style can change whether it's now "the
/// first line", which `first_line_heading` cares about), so a single pass doesn't always reach a
/// fully-fixed result. Matches ESLint's own `--fix`, which iterates up to the same count.
const MAX_FIX_PASSES: usize = 10;

/// Applies every diagnostic with a fix (built-in or custom-rule) to `content`, repeating up to
/// [`MAX_FIX_PASSES`] times until a pass makes no further change, so fixing one diagnostic that
/// exposes another converges in a single call instead of requiring the caller to re-invoke.
///
/// Returns the final text, the total number of fixes applied across every pass that actually
/// changed something (a pass whose fixes were all dropped as no-op/overlapping — see
/// [`mq_content_lint::fix::apply_fixes`] — doesn't count, since nothing was fixed), and the
/// diagnostics for the *final* text — the same [`lint_items`] a caller would get by linting the
/// returned text again, computed once already as a side effect of confirming convergence, so a
/// caller that also needs a diagnostic report shouldn't re-lint from scratch.
fn fix_source(content: &str, linter: &Linter, config: &LintConfig) -> io::Result<(String, usize, Vec<ReportItem>)> {
    let mut current = content.to_string();
    let mut total_fixed = 0;
    for _ in 0..MAX_FIX_PASSES {
        let (fixed, items) = fix_source_once(&current, linter, config)?;
        let count = items.iter().filter(|item| item.fix().is_some()).count();
        if count == 0 || fixed == current {
            return Ok((current, total_fixed, items));
        }
        total_fixed += count;
        current = fixed;
    }
    // Gave up after MAX_FIX_PASSES without reaching a fixed point: the last fix_source_once call
    // above linted the pass *before* `current`'s last update, so its diagnostics are stale for
    // the text we're about to return — one more lint-only pass keeps the two in sync.
    let items = lint_items(&current, linter, config)?;
    Ok((current, total_fixed, items))
}

/// One lint-and-fix pass: applies every diagnostic with a fix to `content`, returning the
/// rewritten text alongside the diagnostics that pass computed (reused by [`fix_source`] rather
/// than re-linting). Diagnostics are not recomputed within this single pass — see [`fix_source`],
/// which loops this to convergence.
fn fix_source_once(content: &str, linter: &Linter, config: &LintConfig) -> io::Result<(String, Vec<ReportItem>)> {
    let items = lint_items(content, linter, config)?;
    let fixes: Vec<mq_content_lint::Fix> = items.iter().filter_map(|item| item.fix().cloned()).collect();
    Ok((mq_content_lint::fix::apply_fixes(content, &fixes), items))
}

/// Directory names that are never worth descending into when discovering Markdown files,
/// regardless of `.gitignore`/`.mq-content-lintignore` contents.
const SKIP_DIR_NAMES: &[&str] = &["node_modules", "target"];

/// Custom ignore-file name consulted alongside `.gitignore` (same gitignore glob syntax) when
/// walking a directory — for excluding paths a project doesn't want reported on `mq-content-lint
/// .` but doesn't want (or can't put, if it's tracked) in `.gitignore`.
const IGNORE_FILE_NAME: &str = ".mq-content-lintignore";

/// Resolves a CLI file argument to concrete Markdown file paths, appending them to `out`.
///
/// A directory is searched recursively for `.md`/`.markdown` files, skipping: dotfiles/dotdirs,
/// [`SKIP_DIR_NAMES`], anything matched by a `.gitignore`/`.git/info/exclude`/global gitignore or
/// [`IGNORE_FILE_NAME`] file found along the way, and anything matched by `config.ignore`.
/// Anything else — an explicit file argument — is kept as-is regardless of extension or ignore
/// status (so `README` or a nonexistent path still reaches the caller's `read_to_string` and
/// produces a clear error, and a file named directly on the command line is always linted even if
/// it's `.gitignore`d), matching how `git add <path>` or `eslint <path>` treat an explicit
/// argument.
fn collect_markdown_files(path: &Path, config: &LintConfig, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_dir() {
        walk_dir_for_markdown(path, config, out)
    } else {
        out.push(path.to_path_buf());
        Ok(())
    }
}

fn walk_dir_for_markdown(dir: &Path, config: &LintConfig, out: &mut Vec<PathBuf>) -> io::Result<()> {
    // Patterns are matched relative to `dir` itself (the directory this walk started from), not
    // the process's current directory — keeps this function independent of global process state.
    let mut overrides_builder = ignore::gitignore::GitignoreBuilder::new(dir);
    for pattern in &config.ignore {
        overrides_builder
            .add_line(None, pattern)
            .map_err(|e| io::Error::other(format!("invalid `ignore` pattern {pattern:?}: {e}")))?;
    }
    let overrides = overrides_builder
        .build()
        .map_err(|e| io::Error::other(format!("building `ignore` patterns: {e}")))?;

    let mut builder = ignore::WalkBuilder::new(dir);
    builder.add_custom_ignore_filename(IGNORE_FILE_NAME);
    builder.filter_entry(move |entry| {
        let name = entry.file_name().to_string_lossy();
        if SKIP_DIR_NAMES.contains(&name.as_ref()) {
            return false;
        }
        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
        !overrides.matched(entry.path(), is_dir).is_ignore()
    });

    for entry in builder.build() {
        let entry = entry.map_err(|e| io::Error::other(format!("walking {}: {}", dir.display(), e)))?;
        let path = entry.path();
        if entry.file_type().is_some_and(|t| t.is_file()) && is_markdown_file(path) {
            out.push(path.to_path_buf());
        }
    }
    Ok(())
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

/// Prints `shell`'s completion script for this CLI to `w` (`--generate-completions <shell>`).
/// Generated from the same [`Cli`] clap parses with, so a new flag shows up in completions for
/// free — nothing here needs updating when the CLI's arguments change.
fn generate_completions(shell: clap_complete::Shell, w: &mut impl Write) -> io::Result<()> {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    clap_complete::generate(shell, &mut command, name, w);
    Ok(())
}

/// Prints a JSON Schema (draft-07) for `mq-content-lint.toml` (`--print-json-schema`). Rule
/// names come from [`RuleId::ALL`], so a new built-in rule shows up here automatically; a rule's
/// own option keys (`limit`, `style`, ...) aren't individually typed — `RuleOptions` reads them
/// dynamically per rule, so there's no single schema to generate them from — a rule's table just
/// allows any additional properties.
fn print_json_schema(w: &mut impl Write) -> io::Result<()> {
    let severity_enum = serde_json::json!({
        "type": "string",
        "enum": ["info", "warning", "error"],
    });

    let rule_value = serde_json::json!({
        "description": "`true`/`false` to enable/disable at the rule's default severity, a severity string to enable at that severity, or a table for rule-specific options.",
        "oneOf": [
            {"type": "boolean"},
            severity_enum.clone(),
            {
                "type": "object",
                "properties": {
                    "enabled": {"type": "boolean"},
                    "severity": severity_enum.clone(),
                },
                "additionalProperties": true,
            },
        ],
    });

    let rule_properties: serde_json::Map<String, serde_json::Value> = RuleId::ALL
        .iter()
        .map(|id| (id.as_str().to_string(), rule_value.clone()))
        .collect();

    let schema = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://github.com/harehare/mq-content-lint/schema.json",
        "title": "mq-content-lint configuration",
        "description": "Schema for mq-content-lint.toml. Generated by `mq-content-lint --print-json-schema` — regenerate after upgrading if new rules were added.",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "rules": {
                "type": "object",
                "description": "Per-rule settings, keyed by rule id.",
                "properties": rule_properties,
                "additionalProperties": false,
            },
            "front_matter": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "required_keys": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Front matter keys every document must have (checked by the missing_front_matter_key rule).",
                    },
                },
            },
            "custom_rules": {
                "type": "array",
                "description": "User-defined lint rules expressed as mq queries.",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "query", "message"],
                    "properties": {
                        "id": {"type": "string", "description": "A stable identifier, shown as the diagnostic's rule id."},
                        "query": {"type": "string", "description": "An mq query run against the document. See https://mqlang.org."},
                        "message": {"type": "string", "description": "The diagnostic text shown for every match."},
                        "severity": severity_enum,
                        "fix": {"type": "string", "description": "An optional mq expression producing the fixed text for a matched node."},
                    },
                },
            },
            "ignore": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Gitignore-syntax glob patterns for files/directories the directory walk should skip.",
            },
        },
    });

    writeln!(
        w,
        "{}",
        serde_json::to_string_pretty(&schema).map_err(io::Error::other)?
    )
}

/// Prints a roff man page for this CLI to `w` (`--generate-man-page`), generated from the same
/// [`Cli`] clap parses with — like [`generate_completions`], a new flag documents itself for free.
fn generate_man_page(w: &mut impl Write) -> io::Result<()> {
    clap_mangen::Man::new(Cli::command()).render(w)
}

fn list_rules(w: &mut impl Write) -> io::Result<()> {
    let mut rules = mq_content_lint::rules::all_rules();
    rules.sort_by_key(|r| r.id());
    for rule in &rules {
        let selector = rule
            .id()
            .selector()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string());
        writeln!(
            w,
            "{:<34} {:<8} {:<4} {}",
            rule.id().as_str().bright_cyan(),
            rule.default_severity().to_string(),
            if rule.fixable() { "fix" } else { "-" },
            selector.dimmed(),
        )?;
    }
    Ok(())
}

/// Prints `rule_id`'s description, default severity, mq selector, and configurable options
/// (`--explain <rule-id>`) — a CLI-accessible substitute for browsing rule source or the README's
/// rule table.
fn explain_rule(w: &mut impl Write, rule_id: RuleId) -> io::Result<()> {
    let rules = mq_content_lint::rules::all_rules();
    let rule = rules
        .iter()
        .find(|r| r.id() == rule_id)
        .expect("every RuleId has a matching Rule in all_rules()");

    writeln!(w, "{}", rule_id.as_str().bright_cyan().bold())?;
    writeln!(w, "{}", rule_id.description())?;
    writeln!(w, "default severity: {}", rule.default_severity())?;
    writeln!(w, "fixable:          {}", if rule.fixable() { "yes" } else { "no" })?;
    if let Some(selector) = rule_id.selector() {
        writeln!(w, "mq selector:      {selector}")?;
    }
    if rule.option_keys().is_empty() {
        writeln!(w, "options:          none")?;
    } else {
        writeln!(w, "options:          {}", rule.option_keys().join(", "))?;
    }
    writeln!(
        w,
        "disable it with:  [rules]\n                   {} = false",
        rule_id.as_str()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Parser, ValueEnum};
    use rstest::rstest;

    #[rstest]
    #[case(vec!["mq-content-lint", "test.md"], vec!["test.md"])]
    #[case(vec!["mq-content-lint"], vec![])]
    fn test_cli_parsing(#[case] args: Vec<&str>, #[case] expected_files: Vec<&str>) {
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(
            cli.files,
            expected_files.into_iter().map(PathBuf::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_cli_version_flag_is_wired_up() {
        // clap intercepts --version/--help before normal parsing, surfacing it as a "parse
        // error" whose kind says which — this used to be a plain UnknownArgument error since the
        // Cli command had no `version` attribute at all.
        let Err(err) = Cli::try_parse_from(["mq-content-lint", "--version"]) else {
            panic!("--version should not parse as a normal Cli");
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_cli_generate_completions_parses_a_shell() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--generate-completions", "zsh"]).unwrap();
        assert_eq!(cli.generate_completions, Some(clap_complete::Shell::Zsh));
        let cli = Cli::try_parse_from(["mq-content-lint"]).unwrap();
        assert_eq!(cli.generate_completions, None);
    }

    #[test]
    fn test_cli_generate_completions_rejects_an_unknown_shell() {
        let result = Cli::try_parse_from(["mq-content-lint", "--generate-completions", "cmd.exe"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_completions_writes_a_non_empty_script_for_every_supported_shell() {
        for shell in clap_complete::Shell::value_variants() {
            let mut out = Vec::new();
            generate_completions(*shell, &mut out).unwrap();
            assert!(!out.is_empty(), "{shell:?} produced no completion script");
            let text = String::from_utf8(out).unwrap();
            assert!(
                text.contains("mq-content-lint"),
                "{shell:?}'s script should reference the binary name"
            );
        }
    }

    #[test]
    fn test_cli_generate_man_page_parses() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--generate-man-page"]).unwrap();
        assert!(cli.generate_man_page);
        let cli = Cli::try_parse_from(["mq-content-lint"]).unwrap();
        assert!(!cli.generate_man_page);
    }

    #[test]
    fn test_generate_man_page_writes_a_non_empty_roff_document() {
        let mut out = Vec::new();
        generate_man_page(&mut out).unwrap();
        assert!(!out.is_empty());
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains(".TH"),
            "roff output should start with a title heading macro"
        );
        assert!(text.contains("mq-content-lint"));
    }

    #[test]
    fn test_cli_print_json_schema_parses() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--print-json-schema"]).unwrap();
        assert!(cli.print_json_schema);
        let cli = Cli::try_parse_from(["mq-content-lint"]).unwrap();
        assert!(!cli.print_json_schema);
    }

    #[test]
    fn test_print_json_schema_is_valid_json_covering_every_rule() {
        let mut out = Vec::new();
        print_json_schema(&mut out).unwrap();
        let schema: serde_json::Value = serde_json::from_slice(&out).unwrap();

        let rule_properties = schema["properties"]["rules"]["properties"]
            .as_object()
            .expect("rules.properties should be an object");
        assert_eq!(rule_properties.len(), RuleId::ALL.len());
        for id in RuleId::ALL {
            assert!(
                rule_properties.contains_key(id.as_str()),
                "schema is missing rule {}",
                id.as_str()
            );
        }

        assert_eq!(
            schema["properties"]["custom_rules"]["items"]["required"],
            serde_json::json!(["id", "query", "message"])
        );
    }

    #[test]
    fn test_cli_explain_parses_a_rule_id() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--explain", "heading_style"]).unwrap();
        assert_eq!(cli.explain, Some(RuleId::HeadingStyle));
        let cli = Cli::try_parse_from(["mq-content-lint"]).unwrap();
        assert_eq!(cli.explain, None);
    }

    #[test]
    fn test_cli_explain_rejects_an_unknown_rule_id() {
        let result = Cli::try_parse_from(["mq-content-lint", "--explain", "not_a_real_rule"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_explain_rule_prints_description_severity_and_options() {
        let mut out = Vec::new();
        explain_rule(&mut out, RuleId::LineLength).unwrap();
        let text = strip_ansi(&out);
        assert!(text.contains("MD013: line length."));
        assert!(text.contains("default severity: info"));
        assert!(text.contains("fixable:          no"));
        assert!(text.contains("limit"));
        assert!(text.contains("line_length = false"));
    }

    #[test]
    fn test_explain_rule_reports_fixable_yes_for_a_fixable_rule() {
        let mut out = Vec::new();
        explain_rule(&mut out, RuleId::HeadingStyle).unwrap();
        let text = strip_ansi(&out);
        assert!(text.contains("fixable:          yes"));
    }

    #[test]
    fn test_explain_rule_reports_no_options_for_a_rule_with_none() {
        let mut out = Vec::new();
        explain_rule(&mut out, RuleId::FirstLineHeading).unwrap();
        let text = strip_ansi(&out);
        assert!(text.contains("options:          none"));
    }

    fn strip_ansi(bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes);
        let mut result = String::with_capacity(text.len());
        let mut in_escape = false;
        for ch in text.chars() {
            if in_escape {
                if ch == 'm' {
                    in_escape = false;
                }
            } else if ch == '\u{1b}' {
                in_escape = true;
            } else {
                result.push(ch);
            }
        }
        result
    }

    #[test]
    fn test_cli_disable_rule() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--disable", "image_missing_alt"]).unwrap();
        assert_eq!(cli.disable, vec![RuleId::ImageMissingAlt]);
    }

    #[test]
    fn test_cli_min_severity() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--min-severity", "warning"]).unwrap();
        assert_eq!(cli.min_severity.0, Severity::Warning);
    }

    #[test]
    fn test_cli_min_severity_invalid() {
        let result = Cli::try_parse_from(["mq-content-lint", "--min-severity", "bogus"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_fix_flag() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--fix", "test.md"]).unwrap();
        assert!(cli.fix);
        let cli = Cli::try_parse_from(["mq-content-lint", "test.md"]).unwrap();
        assert!(!cli.fix);
    }

    #[test]
    fn test_cli_diff_flag() {
        let cli = Cli::try_parse_from(["mq-content-lint", "--diff", "test.md"]).unwrap();
        assert!(cli.diff);
        let cli = Cli::try_parse_from(["mq-content-lint", "test.md"]).unwrap();
        assert!(!cli.diff);
    }

    #[rstest]
    #[case(vec!["mq-content-lint"], OutputFormat::Text)]
    #[case(vec!["mq-content-lint", "--format", "text"], OutputFormat::Text)]
    #[case(vec!["mq-content-lint", "--format", "json"], OutputFormat::Json)]
    #[case(vec!["mq-content-lint", "--format", "sarif"], OutputFormat::Sarif)]
    #[case(vec!["mq-content-lint", "--format", "rdjson"], OutputFormat::Rdjson)]
    fn test_cli_format(#[case] args: Vec<&str>, #[case] expected: OutputFormat) {
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.format, expected);
    }

    #[test]
    fn test_cli_format_invalid() {
        let result = Cli::try_parse_from(["mq-content-lint", "--format", "bogus"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_lint_content_filters_by_min_severity() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let diagnostics = lint_content("![](x.png)\n", &linter, &config, Severity::Error).unwrap();
        assert_eq!(diagnostics.len(), 1);
        let none = lint_content("# ok\n", &linter, &config, Severity::Error).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_fix_source_applies_fixes_in_one_pass() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (fixed, count, items) = fix_source("#Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert_eq!(count, 1);
        assert!(
            items.is_empty(),
            "the returned diagnostics describe the *fixed* text, which has nothing left to report"
        );
    }

    #[test]
    fn test_fix_source_converges_across_multiple_passes() {
        // Pass 1 fixes no_missing_space_atx on both headings. Pass 2's blanks_around_headings
        // fixes then each insert their own blank line between the tight H1/H2 pair, producing a
        // *new* diagnostic (no_multiple_blanks) that a single-pass fix would leave behind. Pass 3
        // collapses that back down to one blank line, and pass 4 finds nothing left to fix.
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (fixed, count, _items) = fix_source("#H1\n##H2\nbody\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# H1\n\n## H2\n\nbody\n");
        assert_eq!(count, 6, "2 (pass 1) + 3 (pass 2) + 1 (pass 3)");

        // A single lint-and-fix pass alone is not enough to reach that result.
        let (single_pass, _items) = fix_source_once("#H1\n##H2\nbody\n", &linter, &config).unwrap();
        assert_ne!(single_pass, fixed);
    }

    #[test]
    fn test_fix_source_applies_a_custom_rule_fix() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::from_toml_str(
            r#"
            [[custom_rules]]
            id = "no_todo"
            query = 'select(contains(to_text(), "TODO"))'
            message = "found a TODO marker"
            fix = 'replace("TODO", "DONE")'
            "#,
        )
        .unwrap();
        let (fixed, count, _items) = fix_source("TODO: fix this\n", &linter, &config).unwrap();
        assert_eq!(fixed, "DONE: fix this\n");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_fix_source_is_a_noop_when_nothing_is_fixable() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (fixed, count, items) = fix_source("# Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert_eq!(count, 0);
        assert!(items.is_empty());
    }

    #[test]
    fn test_fix_source_returned_diagnostics_describe_the_final_text_not_the_original() {
        // A rule with no fix (missing_front_matter_key) has to survive in the returned
        // diagnostics even though nothing about it was ever "fixed" — fix_source's returned
        // items are the full remaining report, not just what got fixed.
        let linter = Linter::with_default_rules();
        let config = LintConfig::from_toml_str("[front_matter]\nrequired_keys = [\"title\"]\n").unwrap();
        let (fixed, _count, items) = fix_source("#Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert!(items.iter().any(|item| item.rule_id() == "missing_front_matter_key"));
    }

    #[test]
    fn test_compute_diff_shows_a_unified_diff() {
        let diff = compute_diff("test.md", "#Title\n", "# Title\n");
        assert!(diff.contains("--- a/test.md"));
        assert!(diff.contains("+++ b/test.md"));
        assert!(diff.contains("-#Title"));
        assert!(diff.contains("+# Title"));
    }

    #[test]
    fn test_process_file_diff_mode_does_not_write_the_file() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-diff-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.md");
        std::fs::write(&path, "#Title\n").unwrap();

        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let outcome = process_file(&path, &linter, &config, Severity::Info, false, true).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "#Title\n",
            "file must stay untouched"
        );
        assert!(outcome.diff.is_some());
        assert!(outcome.fix_count.is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_process_file_diff_mode_is_a_noop_when_nothing_is_fixable() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-diff-noop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.md");
        std::fs::write(&path, "# Title\n").unwrap();

        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let outcome = process_file(&path, &linter, &config, Severity::Info, false, true).unwrap();

        assert!(outcome.diff.is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_collect_markdown_files_filters_by_extension() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "# a\n").unwrap();
        std::fs::write(dir.join("b.txt"), "not markdown\n").unwrap();
        std::fs::write(dir.join("sub/c.markdown"), "# c\n").unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join(".hidden/d.md"), "# d\n").unwrap();

        let mut out = Vec::new();
        collect_markdown_files(&dir, &LintConfig::default(), &mut out).unwrap();
        out.sort();

        assert_eq!(out, vec![dir.join("a.md"), dir.join("sub/c.markdown")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_collect_markdown_files_skips_hardcoded_skip_dirs() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-skipdir-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join("node_modules/pkg.md"), "# pkg\n").unwrap();
        std::fs::write(dir.join("a.md"), "# a\n").unwrap();

        let mut out = Vec::new();
        collect_markdown_files(&dir, &LintConfig::default(), &mut out).unwrap();
        out.sort();

        assert_eq!(out, vec![dir.join("a.md")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_collect_markdown_files_respects_configured_ignore_patterns() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-ignore-cfg-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("vendor")).unwrap();
        std::fs::write(dir.join("vendor/lib.md"), "# lib\n").unwrap();
        std::fs::write(dir.join("a.md"), "# a\n").unwrap();

        let config = LintConfig::from_toml_str("ignore = [\"vendor/**\"]\n").unwrap();
        let mut out = Vec::new();
        collect_markdown_files(&dir, &config, &mut out).unwrap();
        out.sort();

        assert_eq!(out, vec![dir.join("a.md")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_collect_markdown_files_respects_a_dot_ignore_file() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-ignorefile-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".mq-content-lintignore"), "skip-me.md\n").unwrap();
        std::fs::write(dir.join("skip-me.md"), "# skip\n").unwrap();
        std::fs::write(dir.join("a.md"), "# a\n").unwrap();

        let mut out = Vec::new();
        collect_markdown_files(&dir, &LintConfig::default(), &mut out).unwrap();
        out.sort();

        assert_eq!(out, vec![dir.join("a.md")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_collect_markdown_files_always_lints_an_explicit_file_argument_even_if_ignored() {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-explicit-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("skip-me.md");
        std::fs::write(&path, "# skip\n").unwrap();
        let config = LintConfig::from_toml_str("ignore = [\"skip-me.md\"]\n").unwrap();

        let mut out = Vec::new();
        collect_markdown_files(&path, &config, &mut out).unwrap();

        assert_eq!(out, vec![path]);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
