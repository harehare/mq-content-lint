<div align="center">
  <img src="assets/logo.svg" style="width: 128px; height: 128px;"/>

<h1>mq-content-lint</h1>

**Lint Markdown content with [mq](https://github.com/harehare/mq) queries.**

[![ci](https://img.shields.io/github/actions/workflow/status/harehare/mq-content-lint/ci.yml?style=flat-square&logo=github-actions&label=ci)](https://github.com/harehare/mq-content-lint/actions/workflows/ci.yml)
[![LICENCE](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

</div>

A content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s `mq-markdown`
AST, the same document model mq's own query engine and `mq-lint` use. `mq-content-lint` checks
the *content* of a document: heading structure, list and table consistency, whitespace,
link/image hygiene, required front matter, with coverage roughly equivalent to
[markdownlint](https://github.com/DavidAnson/markdownlint)'s rule set. It's a separate tool from
[`mq-lint`](https://github.com/harehare/mq/tree/main/crates/mq-lint), which lints `.mq` query
scripts, not Markdown content.

> [!IMPORTANT]
> This project is under active development.

## Why mq-content-lint?

- **Custom rules without writing Rust**: house style rules, required disclaimers, "no TODO left
  in shipped docs", expressed as [mq](https://github.com/harehare/mq) queries in config instead
  of forking a linter. Neither markdownlint nor rumdl offer this.
- **CI quality gates**: `sarif`/`rdjson` output plugs straight into GitHub code scanning or
  [reviewdog](https://github.com/reviewdog/reviewdog) PR annotations; a non-zero exit code on any
  diagnostic at or above `--min-severity` makes it a drop-in CI check.
- **Live editor feedback**: the bundled LSP server gives hover text, diagnostics, and quick fixes
  as you type, instead of only catching issues at commit or CI time.
- **Migrating from markdownlint**: rule-for-rule coverage of markdownlint's checks, plus the
  ability to express project-specific rules that markdownlint's plugin API can't.
- **Cleaning up LLM-generated Markdown**: docs and READMEs increasingly start as LLM output;
  `--fix` normalizes heading structure, whitespace, and link/list formatting in one pass.

## Features

- **54 built-in rules** covering headings, lists, whitespace, code blocks, links/images, inline
  formatting, tables, and front matter. See `--list-rules` and `--explain <rule-id>`.
- **Custom rules as [mq](https://github.com/harehare/mq) queries**: define project-specific
  checks in config without writing Rust, the one thing neither markdownlint nor rumdl offer.
- **Autofix** via `--fix`, a dry-run diff via `--diff`, and `--watch` to re-lint on save.
- **Cascading TOML config** (like ESLint's), `.editorconfig` integration, `.gitignore`-aware
  directory scanning, and inline `<!-- mq-content-lint-disable ... -->` comments.
- **Machine-readable output**: `json`, `sarif` (GitHub code scanning), and `rdjson` (for
  [reviewdog](https://github.com/reviewdog/reviewdog) inline PR suggestions); `markdown` for a
  table you can drop straight into a PR description or comment.
- **A composite GitHub Action**, a **pre-commit hook**, and an **LSP server**
  (`mq-content-lint-lsp`) for live editor diagnostics and quick fixes.

## Installation

```bash
cargo install mq-content-lint

# Also want the language server? See "Editors" below.
cargo install mq-content-lint --features lsp
```

<details>
<summary>Building from source</summary>

```sh
git clone https://github.com/harehare/mq-content-lint.git
cd mq-content-lint
cargo install --path . --features lsp
```

</details>

## Editor & CI Integrations

| Integration    | Notes                                                                    |
| -------------- | ------------------------------------------------------------------------- |
| VS Code        | [Extension source](./editors/vscode), not yet on the Marketplace, run it from source |
| LSP (any editor) | Point a generic LSP client at `mq-content-lint-lsp` for Markdown files |
| GitHub Actions | Composite action (see below)                                             |
| pre-commit     | `mq-content-lint` / `mq-content-lint-fix` hooks (see below)              |

## Usage

```bash
# Write a starter mq-content-lint.toml (everything commented out, see Configuration)
mq-content-lint --init

# Lint a file, a directory (recursively), or stdin
mq-content-lint README.md
mq-content-lint docs/
cat README.md | mq-content-lint

# Rewrite files in place, or preview the changes as a diff without writing anything
mq-content-lint --fix docs/
mq-content-lint --diff docs/

# Re-lint whenever a watched file changes
mq-content-lint --watch docs/

# Machine-readable output
mq-content-lint --format json docs/ > report.json
mq-content-lint --format sarif docs/ > report.sarif
mq-content-lint --format markdown docs/ > report.md

# Inspect the rule set
mq-content-lint --list-rules
mq-content-lint --explain line_length
```

Exit code is non-zero if any diagnostic at or above `--min-severity` (default `info`) was
reported. When given more than one file, files are read, fixed, and linted in parallel across CPU
cores; output stays sorted by path, so multi-file runs are deterministic to diff or snapshot in
CI.

`--fix` applies every diagnostic with a machine-applicable rewrite, then re-lints and re-fixes
automatically if that exposed a new issue, repeating up to 10 times until a pass makes no further
change. Not every rule can auto-fix; see `--list-rules`'s "Fix?" column or `--explain
<rule-id>`'s `fixable:` line.

### GitHub Actions

```yaml
- uses: harehare/mq-content-lint@v1
  with:
    path: docs/
```

Pass `fix: 'true'` to auto-fix instead of just reporting, or set `format: sarif` and wire the
`sarif-file` output into `github/codeql-action/upload-sarif`. See `action.yml` for the full list
of inputs/outputs.

### pre-commit

```yaml
repos:
  - repo: https://github.com/harehare/mq-content-lint
    rev: v1.0.0 # a tag; see this repo's releases for the latest
    hooks:
      - id: mq-content-lint       # report only
      # - id: mq-content-lint-fix  # or auto-fix on commit instead
```

### Editors

`mq-content-lint-lsp` (installed via `cargo install mq-content-lint --features lsp`) is a
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/) server: live
diagnostics, hover text, and quick-fix code actions in any LSP-capable editor. A [VS Code
extension](./editors/vscode) built on it ships in this repo (not yet on the Marketplace, run it
from source). Other editors can point their generic LSP client at the `mq-content-lint-lsp`
binary for Markdown files.

## Configuration

Drop a `mq-content-lint.toml` file in (or above) the directory you run `mq-content-lint` from.
It's discovered automatically by walking up from the current directory, the same way
`.eslintrc`/`pyproject.toml` are. Config files **cascade**: every `mq-content-lint.toml` found up
to the filesystem root is loaded and layered, closer files winning over farther ones.

```toml
[rules]
heading_hierarchy_skip = "warning"
image_missing_alt = "error"
no_inline_html = false

[rules.line_length]
severity = "warning"
limit = 100

[front_matter]
required_keys = ["title"]
```

See [`mq-content-lint.toml`](./mq-content-lint.toml) in this repo for a fully-commented example,
and `mq-content-lint --print-json-schema` for editor autocomplete/validation.

### Ignoring files

`node_modules`/`target`, dotfiles/dotdirs, and anything excluded by `.gitignore` are always
skipped when linting a directory. A `.mq-content-lintignore` file (gitignore syntax) or an
`ignore = [...]` array in `mq-content-lint.toml` can exclude more. A file named directly on the
command line is always linted regardless.

### Inline disable comments

```markdown
<!-- mq-content-lint-disable line_length -->
This line can be as long as it wants to be now.
<!-- mq-content-lint-enable line_length -->

<!-- mq-content-lint-disable-next-line no_bare_urls -->
See https://example.com for details.
```

`disable`/`enable` take effect from that line onward (or re-enable); `disable-line`/
`disable-next-line` apply to just one line. Omit rule ids to affect every rule.

## Custom rules

A config file can define its own rules as [mq](https://github.com/harehare/mq) queries: every
node a query selects becomes a diagnostic at that node's position, merged into the same report as
the built-ins:

```toml
[[custom_rules]]
id = "no_todo"
query = 'select(contains(to_text(), "TODO"))'
message = "found a TODO marker"
severity = "warning"              # optional, defaults to "warning"
fix = 'replace("TODO", "DONE")'   # optional, makes the rule autofixable
```

`query` runs once per top-level node by default. For a document-wide check (counting duplicate
headings, say), start the query with `nodes` to gather every top-level node into one array first:

```toml
[[custom_rules]]
id = "no_multiple_h1"
query = '''
nodes
| let h1_count = len(compact(.h1))
| if (gt(h1_count, 1)):
    .h1
  end
'''
message = "more than one top-level heading in this document"
severity = "error"
```

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.

## Support

- 🐛 [Report bugs](https://github.com/harehare/mq-content-lint/issues/new)
- 💡 [Request features](https://github.com/harehare/mq-content-lint/issues/new)
- ⭐ [Star the project](https://github.com/harehare/mq-content-lint) if you find it useful!

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for the difference between a
built-in rule and a [custom rule](#custom-rules), and the steps for adding a new built-in rule.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
