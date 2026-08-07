//! Reads the [`max_line_length`](https://editorconfig.org/#supported-properties) property from
//! any `.editorconfig` files a project already has, so `line_length`'s limit follows an existing
//! project-wide convention without needing it repeated in `mq-content-lint.toml`.
//!
//! `.editorconfig` support is intentionally narrow: `max_line_length` is the one property with an
//! unambiguous match to a rule this crate has (`line_length`'s `limit`). Properties like
//! `indent_size` don't map cleanly onto `ul_indent`/`list_indent` — those count spaces per list
//! nesting level, not a single document-wide indent width — so this crate doesn't guess at a
//! mapping for them.

use std::path::Path;

/// Resolves `.editorconfig`'s `max_line_length` as it would apply to a Markdown file in `dir`,
/// walking ancestor directories the same way [`ec4rs`] always does (honoring `root = true`).
/// `None` if no `.editorconfig` sets it, sets it to `off`, or none is found at all.
///
/// Resolved once per lint invocation from a single directory — the same granularity
/// `mq-content-lint.toml` cascading already uses — rather than per linted file, since a real
/// per-file `.editorconfig` lookup (properties can differ by nested directory or glob) isn't
/// meaningfully different in practice for a single property most projects set once at the root,
/// and would require every caller to plumb a file path through where today only a directory is
/// available.
pub(crate) fn max_line_length(dir: &Path) -> Option<usize> {
    // ec4rs resolves properties for a specific file path (matching `.editorconfig` glob sections
    // against its name/extension) without requiring that file to actually exist — this probe
    // name only needs to look like a Markdown file so `[*.md]`/`[*]` sections match.
    let probe = dir.join("mq-content-lint-editorconfig-probe.md");
    let properties = ec4rs::properties_of(&probe).ok()?;
    match properties.get::<ec4rs::property::MaxLineLen>() {
        Ok(ec4rs::property::MaxLineLen::Value(limit)) => Some(limit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-editorconfig-test-{}", uid()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uid() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
    }

    #[test]
    fn reads_max_line_length_from_an_editorconfig_in_the_directory() {
        let dir = tempdir();
        std::fs::write(
            dir.join(".editorconfig"),
            "root = true\n\n[*.md]\nmax_line_length = 72\n",
        )
        .unwrap();

        assert_eq!(max_line_length(&dir), Some(72));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_max_line_length_from_an_ancestor_editorconfig() {
        let dir = tempdir();
        std::fs::write(dir.join(".editorconfig"), "root = true\n\n[*]\nmax_line_length = 100\n").unwrap();
        let nested = dir.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(max_line_length(&nested), Some(100));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn returns_none_when_max_line_length_is_off() {
        let dir = tempdir();
        std::fs::write(dir.join(".editorconfig"), "root = true\n\n[*]\nmax_line_length = off\n").unwrap();

        assert_eq!(max_line_length(&dir), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn returns_none_when_there_is_no_editorconfig() {
        let dir = tempdir();
        assert_eq!(max_line_length(&dir), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_section_that_does_not_match_markdown_files_does_not_apply() {
        let dir = tempdir();
        std::fs::write(
            dir.join(".editorconfig"),
            "root = true\n\n[*.rs]\nmax_line_length = 100\n",
        )
        .unwrap();

        assert_eq!(max_line_length(&dir), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
