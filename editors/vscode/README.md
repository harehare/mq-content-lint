# mq-content-lint for VS Code

A thin [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) client
for [`mq-content-lint-lsp`](../../src/bin/lsp.rs), showing
[mq-content-lint](https://github.com/harehare/mq-content-lint)'s diagnostics inline as you edit
and offering its fixes as quick actions.

## Requirements

The `mq-content-lint-lsp` binary — this extension is a client, it doesn't reimplement any linting
itself. Install it with:

```bash
cargo install mq-content-lint --locked --features lsp
```

That installs both `mq-content-lint` (the CLI) and `mq-content-lint-lsp` (the language server) —
`lsp` is opt-in, so don't drop it from the command. If
the server isn't on your `PATH`, set `mqContentLint.serverPath` to its full path.

## Features

- Diagnostics update live as you type or save (the server re-lints on every change), each carrying
  its rule id (e.g. `image_missing_alt`) so the Problems panel shows exactly which rule fired.
- Fixable diagnostics offer a quick fix (lightbulb / <kbd>Cmd</kbd>+<kbd>.</kbd>) that applies
  exactly what `mq-content-lint --fix` would.
- Hovering a diagnostic shows the rule's help text (the same "how to fix this" hint the CLI prints
  below each finding).
- **mq-content-lint: Fix Document** command (Command Palette) applies every available quick fix
  across the whole file in one edit, matching `mq-content-lint --fix`'s single-pass semantics.
- **mq-content-lint: Restart Server** command, and the server restarts automatically whenever an
  `mqContentLint.*` setting changes.
- Config resolution (including [config cascading](../../README.md#configuration)) is handled
  server-side per file, the same as the CLI — there's no separate config-path setting to keep in
  sync. Editing an `mq-content-lint.toml` re-lints every open document automatically (no restart
  needed) whenever VS Code supports watched-file notifications, which it does out of the box.

## Settings

| Setting | Default | Description |
|---|---|---|
| `mqContentLint.enable` | `true` | Enable/disable the language server entirely. |
| `mqContentLint.serverPath` | `"mq-content-lint-lsp"` | Path to the server binary; defaults to resolving it from `PATH`. |

## Development

```bash
cd editors/vscode
npm install
npm run compile   # or `npm run watch`
```

Press F5 in VS Code (with this directory open) to launch an Extension Development Host for manual
testing — make sure `mq-content-lint-lsp` is built and on `PATH` first (`cargo install --path .
--locked` from the repo root, or point `mqContentLint.serverPath` at
`target/debug/mq-content-lint-lsp`). There's no packaged/published build yet — `npm run compile`
plus `F5` is the workflow until this ships to the Marketplace.
