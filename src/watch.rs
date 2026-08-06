//! `--watch`: re-lints whenever a watched file changes, instead of exiting after one pass.

use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use colored::Colorize;
use mq_content_lint::{LintConfig, Linter, Severity};
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};

use crate::{Cli, lint_files};

/// How long to keep coalescing further filesystem events after the first one, before re-linting.
/// A save in most editors (write + rename, or several small writes) fires multiple raw events in
/// quick succession; without this a single save could trigger several redundant lint passes.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Runs an initial lint pass over `cli.files`, then re-runs on every subsequent change to any of
/// them until the process is interrupted (Ctrl+C). Each `.md`/`.markdown` file is watched
/// directly; each directory is watched recursively, so files created after the watch starts are
/// picked up too (`lint_files` re-walks directories on every call rather than reusing a resolved
/// path list, precisely so newly created/deleted files are reflected).
///
/// The return value mirrors [`crate::lint_files`]'s for a single pass — Ctrl+C ends the process
/// directly (there's no clean "last exit code" to hand back from a loop that runs until killed).
pub(crate) fn run(
    cli: &Cli,
    config: &LintConfig,
    linter: &Linter,
    min_severity: Severity,
    w: &mut impl Write,
) -> io::Result<bool> {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })
    .map_err(watch_error)?;

    for path in &cli.files {
        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher.watch(path, mode).map_err(watch_error)?;
    }

    loop {
        // Each pass's had-diagnostics result doesn't outlive the loop — there's no "last exit
        // code" to report back from a watch loop that only ends via Ctrl+C.
        lint_files(cli, config, linter, min_severity, w)?;
        writeln!(w, "\n{}", "watching for changes... (Ctrl+C to stop)".dimmed())?;
        w.flush()?;

        wait_for_relevant_change(&rx)?;
        writeln!(w, "\n{}", "─".repeat(40).dimmed())?;
    }
}

/// Blocks until an event that actually changed a `.md`/`.markdown` file's content (or its
/// existence, or a directory that may have gained/lost one) arrives, then drains anything else
/// that shows up within [`DEBOUNCE`] of it, so a single save triggers exactly one re-lint instead
/// of one per raw filesystem event — a single write typically also fires open/access/close-write
/// events on most platforms, which carry no information a re-lint needs.
fn wait_for_relevant_change(rx: &mpsc::Receiver<notify::Result<notify::Event>>) -> io::Result<()> {
    loop {
        match rx.recv() {
            Ok(Ok(event)) if is_content_change(&event.kind) && event.paths.iter().any(|p| is_relevant(p)) => break,
            Ok(_) => continue,
            Err(_) => return Err(io::Error::other("file watcher disconnected")),
        }
    }
    while rx.recv_timeout(DEBOUNCE).is_ok() {}
    Ok(())
}

/// Excludes pure access/metadata events (a read, a permission or atime-only change) — noisy on
/// some platforms/filesystems and never something `--watch` needs to react to.
fn is_content_change(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Name(_))
    )
}

fn is_relevant(path: &Path) -> bool {
    path.is_dir()
        || path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

fn watch_error(e: notify::Error) -> io::Error {
    io::Error::other(format!("watching for file changes: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, RemoveKind};

    #[test]
    fn content_changes_are_relevant() {
        assert!(is_content_change(&EventKind::Create(CreateKind::File)));
        assert!(is_content_change(&EventKind::Remove(RemoveKind::File)));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Data(DataChange::Any))));
    }

    #[test]
    fn pure_access_and_metadata_events_are_not_relevant() {
        assert!(!is_content_change(&EventKind::Access(AccessKind::Read)));
        assert!(!is_content_change(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
    }

    #[test]
    fn markdown_extensions_are_relevant_paths() {
        assert!(is_relevant(Path::new("a.md")));
        assert!(is_relevant(Path::new("a.MARKDOWN")));
        assert!(!is_relevant(Path::new("a.txt")));
    }
}
