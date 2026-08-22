//! On-disk cache for `--cache`: skips re-linting a file whose content and the effective config
//! haven't changed since the last cached run.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use mq_content_lint::report_item::{CachedDiagnostic, ReportItem};
use serde::{Deserialize, Serialize};

/// Default cache file, alongside `.eslintcache`'s convention of a dotfile in the current
/// directory.
pub const DEFAULT_CACHE_LOCATION: &str = ".mq-content-lint-cache.json";

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    content_hash: u64,
    diagnostics: Vec<CachedDiagnostic>,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    tool_version: String,
    config_fingerprint: u64,
    entries: HashMap<String, CacheEntry>,
}

pub struct LintCache {
    path: PathBuf,
    file: CacheFile,
    dirty: bool,
}

impl LintCache {
    /// Loads the cache at `path`, discarding it (starting fresh) if it's missing, unreadable, or
    /// from a different tool version or config — any of which makes its entries untrustworthy.
    pub fn load(path: PathBuf, config_fingerprint: u64) -> Self {
        let file = std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<CacheFile>(&content).ok())
            .filter(|f| f.tool_version == env!("CARGO_PKG_VERSION") && f.config_fingerprint == config_fingerprint)
            .unwrap_or_else(|| CacheFile {
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                config_fingerprint,
                entries: HashMap::new(),
            });
        Self {
            path,
            file,
            dirty: false,
        }
    }

    /// Cached diagnostics for `path`, if its current `content` matches what was cached.
    pub fn get(&self, path: &Path, content: &str) -> Option<Vec<ReportItem>> {
        let entry = self.file.entries.get(&cache_key(path))?;
        (entry.content_hash == hash_content(content))
            .then(|| entry.diagnostics.iter().cloned().map(ReportItem::from).collect())
    }

    /// Records `content`'s diagnostics for `path`. A no-op (no write, no dirty flag) if this
    /// matches what's already cached, so a run that hits cache for every file doesn't rewrite an
    /// unchanged cache file.
    pub fn set(&mut self, path: &Path, content: &str, diagnostics: &[ReportItem]) {
        let content_hash = hash_content(content);
        let diagnostics: Vec<CachedDiagnostic> = diagnostics.iter().map(CachedDiagnostic::from).collect();
        let key = cache_key(path);
        if self
            .file
            .entries
            .get(&key)
            .is_some_and(|e| e.content_hash == content_hash && e.diagnostics == diagnostics)
        {
            return;
        }
        self.file.entries.insert(
            key,
            CacheEntry {
                content_hash,
                diagnostics,
            },
        );
        self.dirty = true;
    }

    pub fn save(&self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let json = serde_json::to_string(&self.file).map_err(io::Error::other)?;
        std::fs::write(&self.path, json)
    }
}

fn cache_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mq_content_lint::{Range, Severity};

    fn diagnostic(rule_id: &str, message: &str) -> ReportItem {
        ReportItem::Cached(CachedDiagnostic {
            rule_id: rule_id.to_string(),
            severity: Severity::Warning,
            message: message.to_string(),
            help: None,
            range: Some(Range::single_line(1, 1, 5)),
            fix: None,
        })
    }

    fn cache_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mq-content-lint-cache-test-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn get_misses_on_an_empty_cache() {
        let cache = LintCache::load(cache_path("empty"), 1);
        assert!(cache.get(Path::new("a.md"), "content").is_none());
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut cache = LintCache::load(cache_path("roundtrip"), 1);
        cache.set(Path::new("a.md"), "# Title\n", &[diagnostic("no_todo", "found a TODO")]);

        let hit = cache.get(Path::new("a.md"), "# Title\n").unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].rule_id(), "no_todo");
        assert_eq!(hit[0].text(), "found a TODO");
    }

    #[test]
    fn get_misses_once_the_files_content_changes() {
        let mut cache = LintCache::load(cache_path("content-change"), 1);
        cache.set(Path::new("a.md"), "# Title\n", &[diagnostic("no_todo", "found a TODO")]);
        assert!(cache.get(Path::new("a.md"), "# Different\n").is_none());
    }

    #[test]
    fn save_is_a_noop_when_nothing_was_ever_set() {
        let path = cache_path("noop-save");
        let cache = LintCache::load(path.clone(), 1);
        cache.save().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn save_then_load_round_trips_to_disk() {
        let path = cache_path("disk-roundtrip");
        let mut cache = LintCache::load(path.clone(), 42);
        cache.set(Path::new("a.md"), "# Title\n", &[diagnostic("no_todo", "found a TODO")]);
        cache.save().unwrap();

        let reloaded = LintCache::load(path.clone(), 42);
        let hit = reloaded.get(Path::new("a.md"), "# Title\n").unwrap();
        assert_eq!(hit[0].text(), "found a TODO");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_discards_a_cache_from_a_different_config_fingerprint() {
        let path = cache_path("fingerprint-mismatch");
        let mut cache = LintCache::load(path.clone(), 1);
        cache.set(Path::new("a.md"), "# Title\n", &[diagnostic("no_todo", "found a TODO")]);
        cache.save().unwrap();

        let reloaded = LintCache::load(path.clone(), 2);
        assert!(reloaded.get(Path::new("a.md"), "# Title\n").is_none());

        std::fs::remove_file(&path).unwrap();
    }
}
