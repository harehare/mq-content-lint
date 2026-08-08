# mq-content-lint

A content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s `mq-markdown`
AST — the same document model mq's own query engine and `mq-lint` use. `mq-content-lint` checks
the *content* of a document — heading structure, list and table consistency, whitespace,
link/image hygiene, required front matter — with coverage roughly equivalent to
[markdownlint](https://github.com/DavidAnson/markdownlint)'s rule set. It's a separate tool from
[`mq-lint`](https://github.com/harehare/mq/tree/main/crates/mq-lint), which lints `.mq` query
scripts, not Markdown content.

## Features

- **54 built-in rules** covering headings, lists, whitespace, code blocks, links/images, inline
  formatting, tables, and front matter — see `--list-rules` and `--explain <rule-id>`.
- **Custom rules as [mq](https://github.com/harehare/mq) queries** — define project-specific
  checks in config without writing Rust, the one thing neither markdownlint nor rumdl offer.
- **Autofix** via `--fix`, a dry-run diff via `--diff`, and `--watch` to re-lint on save.
- **Cascading TOML config** (like ESLint's), `.editorconfig` integration, `.gitignore`-aware
  directory scanning, and inline `<!-- mq-content-lint-disable ... -->` comments.
- **Machine-readable output**: `json`, `sarif` (GitHub code scanning), and `rdjson` (for
  [reviewdog](https://github.com/reviewdog/reviewdog) inline PR suggestions).
- **A composite GitHub Action**, a **pre-commit hook**, and an **LSP server**
  (`mq-content-lint-lsp`) for live editor diagnostics and quick fixes.

## Install

```bash
cargo install mq-content-lint

# Also want the language server? See "Editors" below.
cargo install mq-content-lint --features lsp
```

## Usage

```bash
# Write a starter mq-content-lint.toml (everything commented out — see Configuration)
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
change. Not every rule can auto-fix — see `--list-rules`'s "Fix?" column or `--explain
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
extension](./editors/vscode) built on it ships in this repo (not yet on the Marketplace — run it
from source). Other editors can point their generic LSP client at the `mq-content-lint-lsp`
binary for Markdown files.

## Configuration

Drop a `mq-content-lint.toml` file in (or above) the directory you run `mq-content-lint` from —
it's discovered automatically by walking up from the current directory, the same way
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

A config file can define its own rules as [mq](https://github.com/harehare/mq) queries — every
node a query selects becomes a diagnostic at that node's position, merged into the same report as
the built-ins:

```toml
[[custom_rules]]
id = "no_todo"
query = 'select(contains(to_text(), "TODO"))'
message = "found a TODO marker"
severity = "warning"              # optional, defaults to "warning"
fix = 'replace("TODO", "DONE")'   # optional — makes the rule autofixable
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

See [CONTRIBUTING.md](./CONTRIBUTING.md) for details on adding a new built-in rule instead.

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.

## License

MIT
