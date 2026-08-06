mod format;
mod watch;

use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::Parser;
use colored::Colorize;
use format::OutputFormat;
use mq_content_lint::report_item::ReportItem;
use mq_content_lint::{LintConfig, Linter, RuleId, Severity};
use rayon::prelude::*;

/// Static content linter for Markdown, built on mq's AST and selectors.
#[derive(Parser)]
#[command(name = "mq-content-lint", about = "Lint Markdown content")]
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

    /// Rewrite files in place, applying every diagnostic with a machine-applicable fix in a
    /// single pass (reads stdin if no files are given, writing the fixed content to stdout).
    /// Diagnostics are not recomputed between fixes; run again to pick up anything a fix
    /// exposed.
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
            let (fixed, _) = fix_source(&content, &linter, &config)?;
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
/// `fix` rewrites the file in place when changed. `diff` computes the same fix but never writes
/// — it captures a unified diff instead, and linting proceeds against the file's original
/// (unwritten) content, matching what's actually on disk. If both are set, `diff` wins (no
/// write); `--fix --diff` is accepted but behaves like `--diff` alone.
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

    let (content, fix_count, file_diff) = if fix || diff {
        let (fixed, count) = fix_source(&original, linter, config)
            .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;
        if fixed == original {
            (original, None, None)
        } else if diff {
            let diff_text = compute_diff(&label, &original, &fixed);
            (original, None, Some(diff_text))
        } else {
            std::fs::write(path, &fixed)
                .map_err(|e| io::Error::other(format!("writing file {}: {}", path.display(), e)))?;
            (fixed, Some(count), None)
        }
    } else {
        (original, None, None)
    };

    let diagnostics = lint_content(&content, linter, config, min_severity)
        .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;

    Ok(FileOutcome {
        label,
        diagnostics,
        fix_count,
        diff: file_diff,
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

/// Parses `content` as Markdown and returns built-in plus custom-rule diagnostics at or above
/// `min_severity`, merged and sorted by position via [`mq_content_lint::report_item::lint`].
fn lint_content(
    content: &str,
    linter: &Linter,
    config: &LintConfig,
    min_severity: Severity,
) -> io::Result<Vec<ReportItem>> {
    let doc: mq_markdown::Markdown = content
        .parse()
        .map_err(|e: miette::Error| io::Error::other(e.to_string()))?;

    let items = mq_content_lint::report_item::lint(&doc, content, linter, config).map_err(io::Error::other)?;
    Ok(items
        .into_iter()
        .filter(|item| item.severity() >= min_severity)
        .collect())
}

/// Applies every diagnostic with a fix (built-in or custom-rule) to `content` in a single pass,
/// returning the rewritten text and how many fixes were applied.
fn fix_source(content: &str, linter: &Linter, config: &LintConfig) -> io::Result<(String, usize)> {
    let doc: mq_markdown::Markdown = content
        .parse()
        .map_err(|e: miette::Error| io::Error::other(e.to_string()))?;

    let items = mq_content_lint::report_item::lint(&doc, content, linter, config).map_err(io::Error::other)?;
    let fixes: Vec<mq_content_lint::Fix> = items.into_iter().filter_map(|item| item.fix().cloned()).collect();

    let fix_count = fixes.len();
    Ok((mq_content_lint::fix::apply_fixes(content, &fixes), fix_count))
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
            "{:<34} {:<8} {}",
            rule.id().as_str().bright_cyan(),
            rule.default_severity().to_string(),
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
    use clap::Parser;
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
        assert!(text.contains("limit"));
        assert!(text.contains("line_length = false"));
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
        let (fixed, count) = fix_source("#Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert_eq!(count, 1);
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
        let (fixed, count) = fix_source("TODO: fix this\n", &linter, &config).unwrap();
        assert_eq!(fixed, "DONE: fix this\n");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_fix_source_is_a_noop_when_nothing_is_fixable() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (fixed, count) = fix_source("# Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert_eq!(count, 0);
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
