mod format;
mod report_item;

use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use clap::Parser;
use colored::Colorize;
use format::OutputFormat;
use mq_content_lint::{LintConfig, Linter, RuleId, Severity};
use rayon::prelude::*;
use report_item::ReportItem;

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

    /// Rewrite files in place, applying every diagnostic with a machine-applicable fix in a
    /// single pass (reads stdin if no files are given, writing the fixed content to stdout).
    /// Diagnostics are not recomputed between fixes; run again to pick up anything a fix
    /// exposed.
    #[arg(long)]
    fix: bool,

    /// Diagnostic output format
    #[arg(long, value_enum, default_value_t)]
    format: OutputFormat,
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
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;

        if cli.fix {
            let (fixed, _) = fix_source(&content, &linter, &config)?;
            write!(w, "{fixed}")?;
            return Ok(false);
        }

        let diagnostics = lint_content(&content, &linter, &config, min_severity)?;
        let had_diagnostics = !diagnostics.is_empty();
        format::write_report(&mut w, cli.format, &[("<stdin>".to_string(), diagnostics)])?;
        return Ok(had_diagnostics);
    }

    let mut paths = Vec::new();
    for arg in &cli.files {
        collect_markdown_files(arg, &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    // Each file's read/fix/lint pipeline is independent (only file I/O and pure computation,
    // no shared mutable state), so it parallelizes cleanly across files with rayon.
    // `par_iter().map(..).collect()` preserves `paths`' order in the output regardless of which
    // thread finishes first, so output stays deterministic; anything that writes to `w` (the
    // "fixed N issues" notices) is deferred to a sequential pass afterward for the same reason.
    let outcomes: Vec<FileOutcome> = paths
        .par_iter()
        .map(|path| -> io::Result<FileOutcome> { process_file(path, &linter, &config, min_severity, cli.fix) })
        .collect::<io::Result<Vec<_>>>()?;

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
    format::write_report(&mut w, cli.format, &results)?;

    Ok(had_diagnostics)
}

/// One file's outcome from [`process_file`]: its diagnostics, and — when `--fix` changed it —
/// how many fixes were applied, for the sequential "fixed N issues in ..." notice.
struct FileOutcome {
    label: String,
    diagnostics: Vec<ReportItem>,
    fix_count: Option<usize>,
}

/// Reads, optionally fixes (writing the result back to disk if changed), and lints a single
/// file. Pure I/O plus computation with no access to shared state, so callers can run this
/// across a rayon `par_iter()` safely.
fn process_file(
    path: &Path,
    linter: &Linter,
    config: &LintConfig,
    min_severity: Severity,
    fix: bool,
) -> io::Result<FileOutcome> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| io::Error::other(format!("reading file {}: {}", path.display(), e)))?;
    let label = path.display().to_string();

    let (content, fix_count) = if fix {
        let (fixed, fix_count) = fix_source(&content, linter, config)
            .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;
        if fixed != content {
            std::fs::write(path, &fixed)
                .map_err(|e| io::Error::other(format!("writing file {}: {}", path.display(), e)))?;
            (fixed, Some(fix_count))
        } else {
            (fixed, None)
        }
    } else {
        (content, None)
    };

    let diagnostics = lint_content(&content, linter, config, min_severity)
        .map_err(|e| io::Error::other(format!("parsing file {}: {}", path.display(), e)))?;

    Ok(FileOutcome {
        label,
        diagnostics,
        fix_count,
    })
}

/// Parses `content` as Markdown and returns built-in plus custom-rule diagnostics at or above
/// `min_severity`, merged and sorted by position (built-in and custom rules share no ordering
/// otherwise, since they run as two separate passes).
fn lint_content(
    content: &str,
    linter: &Linter,
    config: &LintConfig,
    min_severity: Severity,
) -> io::Result<Vec<ReportItem>> {
    let doc: mq_markdown::Markdown = content
        .parse()
        .map_err(|e: miette::Error| io::Error::other(e.to_string()))?;

    let mut items: Vec<ReportItem> = linter
        .run(&doc, content, config)
        .into_iter()
        .filter(|d| d.severity >= min_severity)
        .map(ReportItem::from)
        .collect();

    let custom_diagnostics =
        mq_content_lint::custom_rules::run(&config.custom_rules, &doc).map_err(io::Error::other)?;
    items.extend(
        custom_diagnostics
            .into_iter()
            .filter(|d| d.severity >= min_severity)
            .map(ReportItem::from),
    );

    items.sort_by_key(|item| item.range().map(|r| (r.start_line, r.start_column)));
    Ok(items)
}

/// Applies every diagnostic with a fix to `content` in a single pass, returning the rewritten
/// text and how many fixes were applied.
fn fix_source(content: &str, linter: &Linter, config: &LintConfig) -> io::Result<(String, usize)> {
    let doc: mq_markdown::Markdown = content
        .parse()
        .map_err(|e: miette::Error| io::Error::other(e.to_string()))?;
    let fixes: Vec<mq_content_lint::Fix> = linter
        .run(&doc, content, config)
        .into_iter()
        .filter_map(|d| d.fix)
        .collect();
    let fix_count = fixes.len();
    Ok((mq_content_lint::fix::apply_fixes(content, &fixes), fix_count))
}

/// Directory names that are never worth descending into when discovering Markdown files.
const SKIP_DIR_NAMES: &[&str] = &["node_modules", "target", ".git"];

/// Resolves a CLI file argument to concrete Markdown file paths, appending them to `out`.
///
/// A directory is searched recursively for `.md`/`.markdown` files (skipping dotfiles/dotdirs
/// and [`SKIP_DIR_NAMES`]). Anything else — an explicit file argument — is kept as-is regardless
/// of extension (so `README` or a nonexistent path still reaches the caller's `read_to_string`
/// and produces a clear error, rather than this function silently dropping it).
fn collect_markdown_files(path: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_dir() {
        walk_dir_for_markdown(path, out)
    } else {
        out.push(path.to_path_buf());
        Ok(())
    }
}

fn walk_dir_for_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIP_DIR_NAMES.contains(&name.as_ref()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_dir_for_markdown(&path, out)?;
        } else if is_markdown_file(&path) {
            out.push(path);
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
    fn test_fix_source_is_a_noop_when_nothing_is_fixable() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (fixed, count) = fix_source("# Title\n", &linter, &config).unwrap();
        assert_eq!(fixed, "# Title\n");
        assert_eq!(count, 0);
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
        collect_markdown_files(&dir, &mut out).unwrap();
        out.sort();

        assert_eq!(out, vec![dir.join("a.md"), dir.join("sub/c.markdown")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
