# mq-content-lint for VS Code

Runs [mq-content-lint](https://github.com/harehare/mq-content-lint) against Markdown files and
shows its diagnostics inline, using the CLI's `--format json` output.

## Requirements

The `mq-content-lint` binary itself — this extension shells out to it, it doesn't reimplement any
linting. Install it with:

```bash
cargo install mq-content-lint
```

If it isn't on your `PATH`, set `mqContentLint.executablePath` to its full path.

## Features

- Lints every open Markdown document on open and on save (configurable to re-lint on every
  keystroke instead).
- **mq-content-lint: Fix Document** command (Command Palette) runs `--fix` on the active file,
  saving first if it has unsaved changes, then re-lints.
- Diagnostics carry the rule id (e.g. `image_missing_alt`) as their code, so hovering or checking
  the Problems panel shows exactly which rule fired.

## Settings

| Setting | Default | Description |
|---|---|---|
| `mqContentLint.enable` | `true` | Enable/disable linting entirely. |
| `mqContentLint.executablePath` | `"mq-content-lint"` | Path to the binary; defaults to resolving it from `PATH`. |
| `mqContentLint.configPath` | `""` | Explicit `mq-content-lint.toml` path. Left empty, the CLI auto-discovers one (see the main README's config cascading). |
| `mqContentLint.run` | `"onSave"` | `"onSave"` or `"onType"`. |

## Development

```bash
cd editors/vscode
npm install
npm run compile   # or `npm run watch`
```

Press F5 in VS Code (with this directory open) to launch an Extension Development Host for manual
testing. There's no packaged/published build yet — `npm run compile` plus `F5` is the workflow
until this ships to the Marketplace.
