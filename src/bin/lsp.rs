//! `mq-content-lint-lsp`: a Language Server Protocol front end for mq-content-lint.
//!
//! Talks LSP over stdio (the standard transport every LSP client — VS Code, Neovim, Helix, Zed,
//! ...— knows how to launch a server with), reusing the library directly rather than shelling out
//! to the `mq-content-lint` binary and parsing its JSON output. Diagnostics are published on
//! open/change/save; hovering one shows its rule's help text, and one with a machine-applicable
//! fix is offered back as a quick-fix `textDocument/codeAction`. Config resolution re-runs for
//! every open document whenever any `mq-content-lint.toml` on disk changes, so editing one takes
//! effect without restarting the server.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use mq_content_lint::report_item::{self, ReportItem};
use mq_content_lint::{Fix, LintConfig, Linter};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[tokio::main]
async fn main() {
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(|client| Backend {
        client,
        linter: Linter::with_default_rules(),
        documents: RwLock::new(HashMap::new()),
        supports_watched_files: AtomicBool::new(false),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// One published diagnostic plus the data needed to answer later requests about it without
/// re-linting: `code_action` needs the fix, `hover` needs the help text.
struct DiagnosticEntry {
    diagnostic: Diagnostic,
    fix: Option<Fix>,
    help: Option<String>,
}

/// A document's text and its most recent lint pass.
struct DocumentState {
    text: String,
    /// Parallel to the diagnostics most recently published for this document — index i's entry
    /// belongs to diagnostic i, and that index is what's stashed in the diagnostic's `data` field
    /// for `code_action`/`hover` to recover.
    entries: Vec<DiagnosticEntry>,
}

struct Backend {
    client: Client,
    linter: Linter,
    documents: RwLock<HashMap<Url, DocumentState>>,
    /// Whether the client declared `workspace.didChangeWatchedFiles.dynamicRegistration` support
    /// in its `initialize` request — set there, read in `initialized` before asking the client to
    /// watch `mq-content-lint.toml` files.
    supports_watched_files: AtomicBool,
}

impl Backend {
    async fn lint_and_publish(&self, uri: Url, text: String, version: Option<i32>) {
        let config = self.resolve_config(&uri).await;
        let entries = lint_to_lsp(&text, &self.linter, &config);
        let diagnostics: Vec<Diagnostic> = entries.iter().map(|e| e.diagnostic.clone()).collect();

        if let Ok(mut documents) = self.documents.write() {
            documents.insert(uri.clone(), DocumentState { text, entries });
        }

        self.client.publish_diagnostics(uri, diagnostics, version).await;
    }

    /// Re-lints every currently open document against its own (possibly just-changed) config —
    /// used when a watched `mq-content-lint.toml` changes, since any open document's effective
    /// config could depend on it.
    async fn relint_open_documents(&self) {
        let open: Vec<(Url, String)> = self
            .documents
            .read()
            .map(|documents| {
                documents
                    .iter()
                    .map(|(uri, state)| (uri.clone(), state.text.clone()))
                    .collect()
            })
            .unwrap_or_default();

        for (uri, text) in open {
            self.lint_and_publish(uri, text, None).await;
        }
    }

    /// Discovers `mq-content-lint.toml` starting from the document's directory — the same
    /// cascading discovery the CLI uses (see [`LintConfig::discover`]). Falls back to defaults
    /// (logged to the client) on a config error, or if the URI isn't a `file://` URI at all.
    async fn resolve_config(&self, uri: &Url) -> LintConfig {
        let Ok(path) = uri.to_file_path() else {
            return LintConfig::default();
        };
        let Some(dir) = path.parent().map(PathBuf::from) else {
            return LintConfig::default();
        };
        match LintConfig::discover(&dir) {
            Ok(config) => config,
            Err(err) => {
                self.client
                    .log_message(MessageType::WARNING, format!("mq-content-lint: {err}, using defaults"))
                    .await;
                LintConfig::default()
            }
        }
    }

    /// Asks the client to watch every `mq-content-lint.toml` in the workspace and forward change
    /// notifications, if it declared support for dynamic registration during `initialize`. A
    /// client that doesn't (or that fails the request) just never sends those notifications —
    /// config changes still take effect on the next open/save, so this is a convenience, not a
    /// requirement.
    async fn register_config_watcher(&self) {
        if !self.supports_watched_files.load(Ordering::Relaxed) {
            return;
        }

        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/mq-content-lint.toml".to_string()),
                kind: None,
            }],
        };
        let Ok(register_options) = serde_json::to_value(options) else {
            return;
        };

        let registration = Registration {
            id: "mq-content-lint-config-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(register_options),
        };
        if let Err(err) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("mq-content-lint: could not watch config files: {err}"),
                )
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let supports_watched_files = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.did_change_watched_files.as_ref())
            .and_then(|d| d.dynamic_registration)
            .unwrap_or(false);
        self.supports_watched_files
            .store(supports_watched_files, Ordering::Relaxed);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "mq-content-lint-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.register_config_watcher().await;
        self.client
            .log_message(MessageType::INFO, "mq-content-lint-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if !is_markdown(&params.text_document) {
            return;
        }
        self.lint_and_publish(
            params.text_document.uri,
            params.text_document.text,
            Some(params.text_document.version),
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: exactly one change event carries the entire new document text.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        self.lint_and_publish(
            params.text_document.uri,
            change.text,
            Some(params.text_document.version),
        )
        .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Re-lint using our own cached text rather than `params.text` (only present if the
        // client opts into `includeText`, which we don't require) — full sync keeps it current.
        let text = self
            .documents
            .read()
            .ok()
            .and_then(|documents| documents.get(&params.text_document.uri).map(|d| d.text.clone()));
        if let Some(text) = text {
            self.lint_and_publish(params.text_document.uri, text, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Ok(mut documents) = self.documents.write() {
            documents.remove(&params.text_document.uri);
        }
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        if params.changes.is_empty() {
            return;
        }
        self.relint_open_documents().await;
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri.clone();
        let Ok(documents) = self.documents.read() else {
            return Ok(None);
        };
        let Some(state) = documents.get(&uri) else {
            return Ok(None);
        };

        let actions: Vec<CodeActionOrCommand> = params
            .context
            .diagnostics
            .into_iter()
            .filter_map(|diagnostic| {
                let index = diagnostic_index(&diagnostic)?;
                let fix = state.entries.get(index)?.fix.as_ref()?;
                Some(CodeActionOrCommand::CodeAction(fix_code_action(&uri, &diagnostic, fix)))
            })
            .collect();

        Ok(Some(actions))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let Ok(documents) = self.documents.read() else {
            return Ok(None);
        };
        let Some(state) = documents.get(&uri) else {
            return Ok(None);
        };

        let matches: Vec<&DiagnosticEntry> = state
            .entries
            .iter()
            .filter(|entry| position_in_range(entry.diagnostic.range, position))
            .collect();
        if matches.is_empty() {
            return Ok(None);
        }

        let value = matches
            .iter()
            .map(|entry| hover_text(entry))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(matches[0].diagnostic.range),
        }))
    }
}

fn is_markdown(document: &TextDocumentItem) -> bool {
    document.language_id == "markdown"
}

/// Recovers the index [`to_lsp_diagnostic`] stashed in a diagnostic's `data` field.
fn diagnostic_index(diagnostic: &Diagnostic) -> Option<usize> {
    diagnostic.data.as_ref()?.get("index")?.as_u64().map(|i| i as usize)
}

/// Whether `position` falls within `range`, inclusive of both endpoints — plain `start <=
/// position < end` would never match a zero-width range (an insertion-point diagnostic, e.g.
/// `no_missing_space_atx`'s), which editors still render as hoverable at that single point.
fn position_in_range(range: Range, position: Position) -> bool {
    let after_start = position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character);
    let before_end = position.line < range.end.line
        || (position.line == range.end.line && position.character <= range.end.character);
    after_start && before_end
}

fn hover_text(entry: &DiagnosticEntry) -> String {
    let rule_id = match &entry.diagnostic.code {
        Some(NumberOrString::String(s)) => s.clone(),
        Some(NumberOrString::Number(n)) => n.to_string(),
        None => String::new(),
    };
    match &entry.help {
        Some(help) => format!("**{rule_id}**: {}\n\nhelp: {help}", entry.diagnostic.message),
        None => format!("**{rule_id}**: {}", entry.diagnostic.message),
    }
}

fn fix_code_action(uri: &Url, diagnostic: &Diagnostic, fix: &Fix) -> CodeAction {
    let edit = TextEdit {
        range: to_lsp_range(fix.range),
        new_text: fix.replacement.clone(),
    };
    CodeAction {
        title: format!("mq-content-lint: fix ({})", diagnostic.message),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
            ..WorkspaceEdit::default()
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    }
}

/// Lints `source`, returning one entry per diagnostic (in publish order) — a parse error or a bad
/// custom-rule query becomes a single document-level entry rather than silently producing
/// nothing, mirroring how the CLI surfaces those as hard errors.
fn lint_to_lsp(source: &str, linter: &Linter, config: &LintConfig) -> Vec<DiagnosticEntry> {
    let doc: mq_markdown::Markdown = match source.parse() {
        Ok(doc) => doc,
        Err(err) => return vec![error_entry(format!("{err}"))],
    };

    let items = match report_item::lint(&doc, source, linter, config) {
        Ok(items) => items,
        Err(err) => return vec![error_entry(format!("{err}"))],
    };

    items
        .iter()
        .enumerate()
        .map(|(index, item)| DiagnosticEntry {
            diagnostic: to_lsp_diagnostic(item, index),
            fix: item.fix().cloned(),
            help: item.help(),
        })
        .collect()
}

fn error_entry(message: String) -> DiagnosticEntry {
    DiagnosticEntry {
        diagnostic: Diagnostic {
            severity: Some(DiagnosticSeverity::WARNING),
            source: Some("mq-content-lint".to_string()),
            message: format!("mq-content-lint: {message}"),
            ..Diagnostic::default()
        },
        fix: None,
        help: None,
    }
}

fn to_lsp_diagnostic(item: &ReportItem, index: usize) -> Diagnostic {
    Diagnostic {
        range: item.range().map(to_lsp_range).unwrap_or_default(),
        severity: Some(to_lsp_severity(item.severity())),
        code: Some(NumberOrString::String(item.rule_id().to_string())),
        source: Some("mq-content-lint".to_string()),
        message: item.text(),
        data: Some(serde_json::json!({ "index": index })),
        ..Diagnostic::default()
    }
}

fn to_lsp_severity(severity: mq_content_lint::Severity) -> DiagnosticSeverity {
    match severity {
        mq_content_lint::Severity::Error => DiagnosticSeverity::ERROR,
        mq_content_lint::Severity::Warning => DiagnosticSeverity::WARNING,
        mq_content_lint::Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// mq-content-lint's `Range` is 1-based with columns counted in `char`s; LSP's `Position` is
/// 0-based with characters counted in UTF-16 code units. These coincide for every character
/// inside the Basic Multilingual Plane (which covers virtually all real-world Markdown, CJK
/// included) — only characters outside it (rare emoji, mathematical symbols, ...) would need a
/// proper UTF-16 recount, which this MVP doesn't do.
fn to_lsp_range(range: mq_content_lint::Range) -> Range {
    Range::new(
        Position::new(
            (range.start_line.saturating_sub(1)) as u32,
            (range.start_column.saturating_sub(1)) as u32,
        ),
        Position::new(
            (range.end_line.saturating_sub(1)) as u32,
            (range.end_column.saturating_sub(1)) as u32,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_lsp_range_converts_one_based_to_zero_based() {
        let range = mq_content_lint::Range {
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 5,
        };
        let lsp_range = to_lsp_range(range);
        assert_eq!(lsp_range.start, Position::new(0, 0));
        assert_eq!(lsp_range.end, Position::new(1, 4));
    }

    #[test]
    fn to_lsp_severity_maps_every_variant() {
        assert_eq!(
            to_lsp_severity(mq_content_lint::Severity::Error),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            to_lsp_severity(mq_content_lint::Severity::Warning),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            to_lsp_severity(mq_content_lint::Severity::Info),
            DiagnosticSeverity::INFORMATION
        );
    }

    #[test]
    fn lint_to_lsp_reports_a_builtin_diagnostic_with_its_rule_id_as_code() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let entries = lint_to_lsp("![](x.png)\n", &linter, &config);

        let entry = entries
            .iter()
            .find(|e| e.diagnostic.code == Some(NumberOrString::String("image_missing_alt".to_string())))
            .expect("image_missing_alt should have fired");
        assert!(entry.fix.is_none(), "image_missing_alt has no mechanical fix");
        assert!(entry.help.is_some(), "image_missing_alt should have a help hint");
    }

    #[test]
    fn lint_to_lsp_carries_a_fix_through_for_a_fixable_rule() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let entries = lint_to_lsp("#Title\n", &linter, &config);

        let index = entries
            .iter()
            .position(|e| e.diagnostic.code == Some(NumberOrString::String("no_missing_space_atx".to_string())))
            .expect("no_missing_space_atx should have fired");
        assert!(entries[index].fix.is_some());
        assert_eq!(diagnostic_index(&entries[index].diagnostic), Some(index));
    }

    #[test]
    fn lint_to_lsp_surfaces_a_custom_rule_error_as_a_diagnostic_instead_of_dropping_it() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::from_toml_str(
            r#"
            [[custom_rules]]
            id = "broken"
            query = "this is not valid mq((("
            message = "never fires"
            "#,
        )
        .unwrap();

        let entries = lint_to_lsp("# Title\n", &linter, &config);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].diagnostic.message.contains("broken"));
    }

    #[test]
    fn fix_code_action_builds_a_workspace_edit_at_the_fixs_range() {
        let uri = Url::parse("file:///tmp/test.md").unwrap();
        let diagnostic = Diagnostic {
            message: "no space after `#`".to_string(),
            ..Diagnostic::default()
        };
        let fix = Fix::new(
            mq_content_lint::Range {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 7,
            },
            "# Title",
        );

        let action = fix_code_action(&uri, &diagnostic, &fix);

        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        let edits = &action.edit.unwrap().changes.unwrap()[&uri];
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "# Title");
    }

    #[test]
    fn position_in_range_matches_a_zero_width_range_at_its_single_point() {
        let range = Range::new(Position::new(0, 5), Position::new(0, 5));
        assert!(position_in_range(range, Position::new(0, 5)));
        assert!(!position_in_range(range, Position::new(0, 4)));
        assert!(!position_in_range(range, Position::new(0, 6)));
    }

    #[test]
    fn position_in_range_matches_inside_a_normal_span() {
        let range = Range::new(Position::new(1, 2), Position::new(1, 8));
        assert!(position_in_range(range, Position::new(1, 2)));
        assert!(position_in_range(range, Position::new(1, 5)));
        assert!(position_in_range(range, Position::new(1, 8)));
        assert!(!position_in_range(range, Position::new(1, 9)));
        assert!(!position_in_range(range, Position::new(0, 5)));
    }

    #[test]
    fn hover_text_includes_help_when_present() {
        let entry = DiagnosticEntry {
            diagnostic: Diagnostic {
                code: Some(NumberOrString::String("image_missing_alt".to_string())),
                message: "image `x.png` has no alt text".to_string(),
                ..Diagnostic::default()
            },
            fix: None,
            help: Some("describe the image's content or purpose in the alt text".to_string()),
        };
        let text = hover_text(&entry);
        assert!(text.contains("image_missing_alt"));
        assert!(text.contains("help:"));
    }

    #[test]
    fn hover_text_omits_help_section_when_absent() {
        let entry = DiagnosticEntry {
            diagnostic: Diagnostic {
                code: Some(NumberOrString::String("no_todo".to_string())),
                message: "found a TODO".to_string(),
                ..Diagnostic::default()
            },
            fix: None,
            help: None,
        };
        let text = hover_text(&entry);
        assert!(!text.contains("help:"));
    }
}
