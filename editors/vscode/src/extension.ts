import { execFile } from 'node:child_process';
import * as vscode from 'vscode';

let diagnosticCollection: vscode.DiagnosticCollection;

export function activate(context: vscode.ExtensionContext): void {
  diagnosticCollection = vscode.languages.createDiagnosticCollection('mq-content-lint');
  context.subscriptions.push(diagnosticCollection);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument(lintIfMarkdown),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (getRunMode() === 'onSave') {
        lintIfMarkdown(document);
      }
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (getRunMode() === 'onType') {
        lintIfMarkdown(event.document);
      }
    }),
    vscode.workspace.onDidCloseTextDocument((document) => diagnosticCollection.delete(document.uri)),
    vscode.commands.registerCommand('mqContentLint.fixDocument', fixActiveDocument),
  );

  // Lint whatever Markdown documents are already open when the extension activates, not just
  // ones opened/saved afterward.
  vscode.workspace.textDocuments.forEach(lintIfMarkdown);
}

export function deactivate(): void {
  diagnosticCollection?.dispose();
}

function lintIfMarkdown(document: vscode.TextDocument): void {
  if (document.languageId !== 'markdown' || document.uri.scheme !== 'file' || !isEnabled()) {
    return;
  }
  void lintDocument(document);
}

function isEnabled(): boolean {
  return vscode.workspace.getConfiguration('mqContentLint').get<boolean>('enable', true);
}

function getRunMode(): 'onSave' | 'onType' {
  return vscode.workspace.getConfiguration('mqContentLint').get<'onSave' | 'onType'>('run', 'onSave');
}

function getExecutablePath(): string {
  return vscode.workspace.getConfiguration('mqContentLint').get<string>('executablePath', 'mq-content-lint');
}

function getConfigPath(): string {
  return vscode.workspace.getConfiguration('mqContentLint').get<string>('configPath', '');
}

/** Shape of one file's entry in `mq-content-lint --format json`'s output array. */
interface CliRange {
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
}

interface CliDiagnostic {
  ruleId: string;
  selector: string | null;
  severity: 'info' | 'warning' | 'error';
  message: string;
  help: string | null;
  range: CliRange | null;
}

interface CliFileReport {
  file: string;
  diagnostics: CliDiagnostic[];
}

async function lintDocument(document: vscode.TextDocument): Promise<void> {
  const args = ['--format', 'json'];
  const configPath = getConfigPath();
  if (configPath) {
    args.push('--config', configPath);
  }
  args.push(document.fileName);

  try {
    const { stdout } = await runCli(args, workspaceFolderFor(document));
    const reports: CliFileReport[] = JSON.parse(stdout);
    const diagnostics = (reports[0]?.diagnostics ?? []).map(toVscodeDiagnostic);
    diagnosticCollection.set(document.uri, diagnostics);
  } catch (err) {
    // A spawn failure (missing binary) or a JSON.parse failure (e.g. the document isn't valid
    // Markdown mq-markdown could parse) shouldn't spam a popup on every save — surface it as a
    // single document-level diagnostic instead, which clears itself once the run succeeds.
    diagnosticCollection.set(document.uri, [
      new vscode.Diagnostic(new vscode.Range(0, 0, 0, 0), `mq-content-lint: ${errorMessage(err)}`, vscode.DiagnosticSeverity.Warning),
    ]);
  }
}

function toVscodeDiagnostic(d: CliDiagnostic): vscode.Diagnostic {
  // CLI positions are 1-based; vscode.Position is 0-based.
  const range = d.range
    ? new vscode.Range(
        Math.max(0, d.range.startLine - 1),
        Math.max(0, d.range.startColumn - 1),
        Math.max(0, d.range.endLine - 1),
        Math.max(0, d.range.endColumn - 1),
      )
    : new vscode.Range(0, 0, 0, 0);
  const diagnostic = new vscode.Diagnostic(range, d.message, toSeverity(d.severity));
  diagnostic.source = 'mq-content-lint';
  diagnostic.code = d.ruleId;
  return diagnostic;
}

function toSeverity(severity: CliDiagnostic['severity']): vscode.DiagnosticSeverity {
  switch (severity) {
    case 'error':
      return vscode.DiagnosticSeverity.Error;
    case 'warning':
      return vscode.DiagnosticSeverity.Warning;
    default:
      return vscode.DiagnosticSeverity.Information;
  }
}

async function fixActiveDocument(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== 'markdown' || editor.document.uri.scheme !== 'file') {
    void vscode.window.showWarningMessage('mq-content-lint: no active Markdown file to fix.');
    return;
  }

  const document = editor.document;
  if (document.isDirty) {
    await document.save();
  }

  const args = ['--fix'];
  const configPath = getConfigPath();
  if (configPath) {
    args.push('--config', configPath);
  }
  args.push(document.fileName);

  try {
    // A non-zero exit here just means diagnostics remain after fixing what could be fixed —
    // not a failure of the fix command itself, so the exit code is intentionally ignored.
    await runCli(args, workspaceFolderFor(document));
  } catch (err) {
    void vscode.window.showErrorMessage(`mq-content-lint --fix failed: ${errorMessage(err)}`);
    return;
  }

  await lintDocument(document);
}

function workspaceFolderFor(document: vscode.TextDocument): string | undefined {
  return vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
}

interface CliResult {
  stdout: string;
  /** Process exit code. mq-content-lint exits non-zero when diagnostics were found — expected,
   *  not an error — so callers decide for themselves whether the code matters. */
  code: number;
}

function runCli(args: string[], cwd: string | undefined): Promise<CliResult> {
  const executablePath = getExecutablePath();
  return new Promise((resolve, reject) => {
    execFile(executablePath, args, { cwd, maxBuffer: 10 * 1024 * 1024 }, (error, stdout) => {
      if (error && typeof error.code === 'string') {
        // A spawn-level failure (ENOENT, EACCES, ...) rather than the process running and
        // exiting non-zero, which `error.code` would instead be a number for.
        reject(new Error(error.code === 'ENOENT' ? `executable not found: ${executablePath}` : error.message));
        return;
      }
      resolve({ stdout, code: typeof error?.code === 'number' ? error.code : 0 });
    });
  });
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
