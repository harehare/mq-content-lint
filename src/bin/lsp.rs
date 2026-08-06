//! `mq-content-lint-lsp`: a Language Server Protocol front end for mq-content-lint.
//!
//! Talks LSP over stdio (the standard transport every LSP client — VS Code, Neovim, Helix, Zed,
//! ...— knows how to launch a server with), reusing the library directly rather than shelling out
//! to the `mq-content-lint` binary and parsing its JSON output. Diagnostics are published on
//! open/change/save; a diagnostic with a machine-applicable fix is offered back as a quick-fix
//! `textDocument/codeAction`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

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
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// A document's text and its most recent lint pass, kept so `code_action` can look a
/// client-selected diagnostic's fix back up without re-linting, and so `did_save` can re-lint
/// without the client having to resend the whole document.
struct DocumentState {
    text: String,
    /// Parallel to the `Diagnostic`s most recently published for this document — index i's fix
    /// belongs to diagnostic i, and that index is what's stashed in the diagnostic's `data` field
    /// for `code_action` to recover.
    fixes: Vec<Option<Fix>>,
}

struct Backend {
    client: Client,
    linter: Linter,
    documents: RwLock<HashMap<Url, DocumentState>>,
}

impl Backend {
    async fn lint_and_publish(&self, uri: Url, text: String, version: Option<i32>) {
        let config = self.resolve_config(&uri).await;
        let (diagnostics, fixes) = lint_to_lsp(&text, &self.linter, &config);

        if let Ok(mut documents) = self.documents.write() {
            documents.insert(uri.clone(), DocumentState { text, fixes });
        }

        self.client.publish_diagnostics(uri, diagnostics, version).await;
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
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "mq-content-lint-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
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
                let fix = state.fixes.get(index)?.as_ref()?;
                Some(CodeActionOrCommand::CodeAction(fix_code_action(&uri, &diagnostic, fix)))
            })
            .collect();

        Ok(Some(actions))
    }
}

fn is_markdown(document: &TextDocumentItem) -> bool {
    document.language_id == "markdown"
}

/// Recovers the index [`to_lsp_diagnostic`] stashed in a diagnostic's `data` field.
fn diagnostic_index(diagnostic: &Diagnostic) -> Option<usize> {
    diagnostic.data.as_ref()?.get("index")?.as_u64().map(|i| i as usize)
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

/// Lints `source`, returning the diagnostics to publish and each one's fix (same order, same
/// index) — a parse error or a bad custom-rule query becomes a single document-level diagnostic
/// rather than silently producing nothing, mirroring how the CLI surfaces those as hard errors.
fn lint_to_lsp(source: &str, linter: &Linter, config: &LintConfig) -> (Vec<Diagnostic>, Vec<Option<Fix>>) {
    let doc: mq_markdown::Markdown = match source.parse() {
        Ok(doc) => doc,
        Err(err) => return (vec![error_diagnostic(format!("{err}"))], vec![None]),
    };

    let items = match report_item::lint(&doc, source, linter, config) {
        Ok(items) => items,
        Err(err) => return (vec![error_diagnostic(format!("{err}"))], vec![None]),
    };

    items
        .iter()
        .enumerate()
        .map(|(index, item)| (to_lsp_diagnostic(item, index), item.fix().cloned()))
        .unzip()
}

fn error_diagnostic(message: String) -> Diagnostic {
    Diagnostic {
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("mq-content-lint".to_string()),
        message: format!("mq-content-lint: {message}"),
        ..Diagnostic::default()
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
        let (diagnostics, fixes) = lint_to_lsp("![](x.png)\n", &linter, &config);

        let index = diagnostics
            .iter()
            .position(|d| d.code == Some(NumberOrString::String("image_missing_alt".to_string())))
            .expect("image_missing_alt should have fired");
        assert!(fixes[index].is_none(), "image_missing_alt has no mechanical fix");
    }

    #[test]
    fn lint_to_lsp_carries_a_fix_through_for_a_fixable_rule() {
        let linter = Linter::with_default_rules();
        let config = LintConfig::default();
        let (diagnostics, fixes) = lint_to_lsp("#Title\n", &linter, &config);

        let index = diagnostics
            .iter()
            .position(|d| d.code == Some(NumberOrString::String("no_missing_space_atx".to_string())))
            .expect("no_missing_space_atx should have fired");
        assert!(fixes[index].is_some());
        assert_eq!(diagnostic_index(&diagnostics[index]), Some(index));
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

        let (diagnostics, _) = lint_to_lsp("# Title\n", &linter, &config);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("broken"));
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
}
