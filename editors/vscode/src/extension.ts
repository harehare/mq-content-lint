import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('mqContentLint.fixDocument', fixActiveDocument),
    vscode.commands.registerCommand('mqContentLint.restartServer', restartClient),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('mqContentLint')) {
        void restartClient();
      }
    }),
  );

  void startClient();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}

function isEnabled(): boolean {
  return vscode.workspace.getConfiguration('mqContentLint').get<boolean>('enable', true);
}

function getServerPath(): string {
  return vscode.workspace.getConfiguration('mqContentLint').get<string>('serverPath', 'mq-content-lint-lsp');
}

async function startClient(): Promise<void> {
  if (!isEnabled()) {
    return;
  }

  const serverOptions: ServerOptions = {
    command: getServerPath(),
    args: [],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'markdown' }],
  };

  client = new LanguageClient('mqContentLint', 'mq-content-lint', serverOptions, clientOptions);

  try {
    await client.start();
  } catch (err) {
    void vscode.window.showErrorMessage(
      `mq-content-lint: failed to start mq-content-lint-lsp (${errorMessage(err)}). ` +
        'Install it with `cargo install mq-content-lint --locked` and/or set mqContentLint.serverPath.',
    );
  }
}

/** Stops and restarts the client — used after a relevant setting changes, and as a manual command. */
async function restartClient(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
  await startClient();
}

/**
 * Applies every quick-fix code action available across the whole active document, in one
 * `WorkspaceEdit` — matching the CLI's `--fix` semantics (a single pass over the original
 * content; fixes aren't recomputed against each other, so overlapping fixes can still only take
 * one of them, same as `mq-content-lint --fix`).
 */
async function fixActiveDocument(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== 'markdown') {
    void vscode.window.showWarningMessage('mq-content-lint: no active Markdown document to fix.');
    return;
  }

  const document = editor.document;
  const fullRange = new vscode.Range(0, 0, document.lineCount, 0);

  const actions =
    (await vscode.commands.executeCommand<(vscode.CodeAction | vscode.Command)[]>(
      'vscode.executeCodeActionProvider',
      document.uri,
      fullRange,
      vscode.CodeActionKind.QuickFix.value,
    )) ?? [];

  const combinedEdit = new vscode.WorkspaceEdit();
  let editCount = 0;
  for (const action of actions) {
    if (!(action instanceof vscode.CodeAction) || !action.edit) {
      continue;
    }
    for (const [uri, edits] of action.edit.entries()) {
      for (const edit of edits) {
        combinedEdit.replace(uri, edit.range, edit.newText);
        editCount += 1;
      }
    }
  }

  if (editCount === 0) {
    void vscode.window.showInformationMessage('mq-content-lint: nothing to fix.');
    return;
  }

  const applied = await vscode.workspace.applyEdit(combinedEdit);
  if (!applied) {
    void vscode.window.showErrorMessage('mq-content-lint: some fixes could not be applied (they may overlap).');
  }
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
