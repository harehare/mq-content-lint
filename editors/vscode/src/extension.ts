import * as vscode from 'vscode';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as fsp from 'node:fs/promises';
import * as path from 'node:path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

const REPO = 'harehare/mq-content-lint';
const DEFAULT_SERVER_PATH = 'mq-content-lint-lsp';

/** Rust target triples that release.yml cross-builds mq-content-lint-lsp for, keyed by Node's platform-arch. */
const RELEASE_TARGETS: Record<string, string> = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

let client: LanguageClient | undefined;
let extensionContext: vscode.ExtensionContext;

export function activate(context: vscode.ExtensionContext): void {
  extensionContext = context;
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

function getConfiguredServerPath(): string {
  return vscode.workspace.getConfiguration('mqContentLint').get<string>('serverPath', DEFAULT_SERVER_PATH);
}

/**
 * Resolves the command to launch the language server with. A `serverPath` the user has pointed
 * at something other than the default is used as-is. Otherwise, an existing PATH install (e.g.
 * via `cargo install`) wins; if there isn't one, a prebuilt binary matching this platform is
 * downloaded from the GitHub release matching this extension's version and cached in global
 * storage, so most users never need Rust installed at all.
 */
async function resolveServerCommand(): Promise<string | undefined> {
  const configured = getConfiguredServerPath();
  if (configured !== DEFAULT_SERVER_PATH) {
    return configured;
  }

  if (findOnPath(configured)) {
    return configured;
  }

  return downloadServerBinary();
}

function findOnPath(command: string): string | undefined {
  const exts = process.platform === 'win32' ? (process.env.PATHEXT ?? '.EXE;.CMD;.BAT').split(';') : [''];
  const dirs = (process.env.PATH ?? '').split(path.delimiter);

  for (const dir of dirs) {
    for (const ext of exts) {
      const candidate = path.join(dir, command + ext);
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }
  }

  return undefined;
}

async function downloadServerBinary(): Promise<string | undefined> {
  const target = RELEASE_TARGETS[`${process.platform}-${process.arch}`];
  if (!target) {
    throw new Error(
      `no prebuilt mq-content-lint-lsp for ${process.platform}/${process.arch}; install it with ` +
        '`cargo install mq-content-lint --locked --features lsp` and set mqContentLint.serverPath to it',
    );
  }

  const version = extensionContext.extension.packageJSON.version as string;
  const assetName = `mq-content-lint-lsp-${target}${process.platform === 'win32' ? '.exe' : ''}`;
  const binDir = vscode.Uri.joinPath(extensionContext.globalStorageUri, 'bin', version).fsPath;
  const binPath = path.join(binDir, assetName);

  if (fs.existsSync(binPath)) {
    return binPath;
  }

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: `mq-content-lint: downloading ${assetName}` },
    async () => {
      const releaseUrl = `https://github.com/${REPO}/releases/download/v${version}`;
      const checksums = await fetchText(`${releaseUrl}/checksums.txt`);
      const expected = findChecksum(checksums, assetName);
      const data = await fetchBinary(`${releaseUrl}/${assetName}`);

      if (expected) {
        const actual = crypto.createHash('sha256').update(data).digest('hex');
        if (actual !== expected) {
          throw new Error(`checksum mismatch for ${assetName}: expected ${expected}, got ${actual}`);
        }
      }

      await fsp.mkdir(binDir, { recursive: true });
      const tmpPath = `${binPath}.download`;
      await fsp.writeFile(tmpPath, data, { mode: 0o755 });
      await fsp.rename(tmpPath, binPath);
    },
  );

  return binPath;
}

async function fetchText(url: string): Promise<string> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText} fetching ${url}`);
  }
  return res.text();
}

async function fetchBinary(url: string): Promise<Buffer> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText} fetching ${url}`);
  }
  return Buffer.from(await res.arrayBuffer());
}

/** checksums.txt is `sha256sum` output — lines of `<hash>  <path>`, path prefixed with an artifact directory. */
function findChecksum(checksumsText: string, assetName: string): string | undefined {
  for (const line of checksumsText.split('\n')) {
    const [hash, filePath] = line.trim().split(/\s+/);
    if (hash && filePath && path.basename(filePath) === assetName) {
      return hash;
    }
  }
  return undefined;
}

async function startClient(): Promise<void> {
  if (!isEnabled()) {
    return;
  }

  let command: string;
  try {
    const resolved = await resolveServerCommand();
    if (!resolved) {
      return;
    }
    command = resolved;
  } catch (err) {
    void vscode.window.showErrorMessage(`mq-content-lint: failed to prepare mq-content-lint-lsp (${errorMessage(err)}).`);
    return;
  }

  const serverOptions: ServerOptions = {
    command,
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
        'Install it with `cargo install mq-content-lint --locked --features lsp` and/or set mqContentLint.serverPath.',
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
