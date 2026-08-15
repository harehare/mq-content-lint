//! MD057: a relative link (`[text](./other.md)`) whose target file doesn't exist on disk,
//! resolved relative to the linted file's own directory. A no-op for stdin/in-memory input with
//! no path to resolve against. Only the file part is checked — `other.md#section` resolves as
//! `other.md`, not validated against that file's headings. Root-relative links (`/other.md`) are
//! left alone; there's no single sensible root to resolve them against.

use mq_markdown::Node;

use crate::rules::Rule;
use crate::{Diagnostic, LintConfig, LintMessage, RuleId, Severity};

pub(crate) struct RelativeLinkExists;

/// Whether `url` has a scheme (`https:`, `mailto:`, ...) or is protocol-relative (`//host/...`),
/// making it not a local path this rule should try to resolve.
fn is_external(url: &str) -> bool {
    if url.starts_with("//") {
        return true;
    }
    match url.find(':') {
        Some(colon) => {
            let scheme = &url[..colon];
            !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

impl Rule for RelativeLinkExists {
    fn id(&self) -> RuleId {
        RuleId::RelativeLinkExists
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check(
        &self,
        doc: &mq_markdown::Markdown,
        _source: &str,
        _config: &LintConfig,
        path: Option<&std::path::Path>,
    ) -> Vec<Diagnostic> {
        let Some(path) = path else { return Vec::new() };
        let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));

        let mut diagnostics = Vec::new();
        crate::walk::walk(doc.nodes.iter(), &mut |node| {
            let Node::Link(link) = node else { return };
            let url = link.url.as_str();
            if url.is_empty() || url.starts_with('#') || url.starts_with('/') || is_external(url) {
                return;
            }
            let file_part = url.split('#').next().unwrap_or(url);
            if file_part.is_empty() || base_dir.join(file_part).exists() {
                return;
            }

            let mut diagnostic = Diagnostic::new(
                LintMessage::RelativeLinkExists { path: url.to_string() },
                self.default_severity(),
            );
            if let Some(position) = link.position.clone() {
                diagnostic = diagnostic.with_range(position);
            }
            diagnostics.push(diagnostic);
        });
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mq-content-lint-relative-link-exists-test-{}", uid()));
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

    fn run(markdown: &str, path: Option<&std::path::Path>) -> Vec<Diagnostic> {
        let doc: mq_markdown::Markdown = markdown.parse().unwrap();
        RelativeLinkExists.check(&doc, markdown, &LintConfig::default(), path)
    }

    #[test]
    fn no_diagnostics_when_the_linted_file_has_no_path() {
        assert!(run("[text](missing.md)\n", None).is_empty());
    }

    #[test]
    fn no_diagnostics_for_an_existing_relative_file() {
        let dir = tempdir();
        std::fs::write(dir.join("other.md"), "# Other\n").unwrap();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](other.md)\n", Some(&doc_path)).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn flags_a_relative_link_to_a_missing_file() {
        let dir = tempdir();
        let doc_path = dir.join("doc.md");

        let diagnostics = run("[text](missing.md)\n", Some(&doc_path));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            LintMessage::RelativeLinkExists {
                path: "missing.md".to_string()
            }
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn strips_a_fragment_before_checking_existence() {
        let dir = tempdir();
        std::fs::write(dir.join("other.md"), "# Other\n").unwrap();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](other.md#section)\n", Some(&doc_path)).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_fragment_only_links() {
        let dir = tempdir();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](#section)\n", Some(&doc_path)).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_root_relative_links() {
        let dir = tempdir();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](/other.md)\n", Some(&doc_path)).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_external_urls() {
        let dir = tempdir();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](https://example.com/missing)\n", Some(&doc_path)).is_empty());
        assert!(run("[text](mailto:someone@example.com)\n", Some(&doc_path)).is_empty());
        assert!(run("[text](//example.com/missing)\n", Some(&doc_path)).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolves_a_subdirectory_relative_to_the_linted_files_own_directory() {
        let dir = tempdir();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/target.md"), "# Target\n").unwrap();
        let doc_path = dir.join("doc.md");

        assert!(run("[text](sub/target.md)\n", Some(&doc_path)).is_empty());
        assert_eq!(run("[text](sub/nope.md)\n", Some(&doc_path)).len(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
