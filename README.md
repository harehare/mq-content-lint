# mq-content-lint

Static content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s
`mq-markdown` AST — the same document model mq's own query engine and `mq-lint` use, reused here
instead of hand-rolling another Markdown parser.

`mq-content-lint` checks the *content* of a document — heading structure, list and table
consistency, whitespace, link/image hygiene, required front matter — comprehensive coverage of
[markdownlint](https://github.com/DavidAnson/markdownlint)'s rule set, expressed against mq's
node model instead of a bespoke rule engine. It is a separate tool from
[`mq-lint`](https://github.com/harehare/mq/tree/main/crates/mq-lint), which lints `.mq` query
scripts, not Markdown content. Beyond the built-in rules, config files can also define their own
[custom rules](#custom-rules) as mq queries.

## Install

```bash
cargo install mq-content-lint
```

Also want the [language server](#editors) (`mq-content-lint-lsp`)? Add `--features lsp` to install
both in one go — see [Editors](#editors).

## Usage

```bash
# Lint one file
mq-content-lint README.md

# Lint a directory recursively (.md / .markdown files; dotfiles/dotdirs, node_modules,
# target, and .git are skipped)
mq-content-lint docs/

# Read from stdin
cat README.md | mq-content-lint

# Rewrite files in place, applying every diagnostic with a machine-applicable fix
mq-content-lint --fix docs/

# Preview what --fix would change, as a unified diff, without writing anything
mq-content-lint --diff docs/

# Re-lint automatically whenever a watched file changes (Ctrl+C to stop)
mq-content-lint --watch docs/

# Machine-readable output
mq-content-lint --format json docs/ > report.json
mq-content-lint --format sarif docs/ > report.sarif

# List built-in rules, their default severity, and the mq selector each corresponds to
mq-content-lint --list-rules
```

Exit code is non-zero if any diagnostic at or above `--min-severity` (default `info`, i.e. "any
diagnostic at all") was reported — the same convention `mq-lint` uses, so both tools fail CI the
same way. Pass `--min-severity error` to only fail on errors.

When given more than one file, they're read, (optionally) fixed, and linted in parallel across
CPU cores; output stays in the same order regardless (sorted by path), so multi-file runs are
deterministic to diff or snapshot in CI.

### Autofix

`--fix` applies every diagnostic with a machine-applicable rewrite in a single pass over the
original source — diagnostics are **not** recomputed between individual fixes, so a fix that
exposes a new issue (or a rule whose fix would have overlapped another rule's fix on the same
span) needs a second `--fix` run to pick up. Rules where there's no single unambiguous rewrite —
no reasonable default alt text, no way to invent required front matter content, an ordered-list
prefix bug that could mean either "insert here" or "renumber" — never populate a fix; see the
"Fix?" column below. A [custom rule](#custom-rules) is fixable too, if it's configured with a
`fix` expression.

Not sure what `--fix` would do before it does it? `--diff` computes the exact same fixes but never
writes — files (and stdin's fixed content) stay untouched, and a unified diff is printed to stdout
instead. It works standalone (no need to pass `--fix` too) and exits non-zero if anything would
change, so `mq-content-lint --diff docs/` doubles as a CI check for "is everything already
formatted."

### Watch mode

`--watch` runs an initial pass and then keeps running, re-linting whenever a watched file changes,
until interrupted (Ctrl+C). It requires at least one file/directory argument (there's no sense
watching stdin) and combines with `--fix`/`--diff` to re-fix or re-preview on every save. A
directory argument is watched recursively, so `.md`/`.markdown` files created after the watch
starts are picked up too.

### GitHub Actions

This repo ships its own composite action (`action.yml`) — it installs `mq-content-lint` (cached,
and skipped entirely if a step earlier in the job already put it on `PATH`) and runs it, so a
workflow doesn't need to hand-roll the install step:

```yaml
- uses: harehare/mq-content-lint@v1
  with:
    path: docs/
```

Pass `fix: 'true'` to auto-fix instead of just reporting, or wire the `sarif-file` output into
GitHub code scanning:

```yaml
- uses: harehare/mq-content-lint@v1
  id: lint
  with:
    path: docs/
    format: sarif
  continue-on-error: true
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: ${{ steps.lint.outputs.sarif-file }}
```

See `action.yml`'s `inputs`/`outputs` for the full list (`config`, `min-severity`, `version`, ...).
Prefer to install manually instead? The equivalent without the action:

```yaml
- name: Install mq-content-lint
  run: cargo install mq-content-lint --locked
- name: Lint docs
  run: mq-content-lint --format sarif docs/ > mq-content-lint.sarif
  continue-on-error: true
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: mq-content-lint.sarif
```

### pre-commit

This repo is also a [pre-commit](https://pre-commit.com) hook repo (`.pre-commit-hooks.yaml`).
Add it to a project's `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/harehare/mq-content-lint
    rev: v1.0.0 # a tag; see the repo's releases for the latest
    hooks:
      - id: mq-content-lint       # report only
      # - id: mq-content-lint-fix  # or auto-fix on commit instead
```

Both hooks use `language: rust`, so pre-commit builds `mq-content-lint` straight from this repo at
the pinned `rev` the first time the hook runs (cached after that) — nothing to install manually,
and no dependency on a crates.io release existing yet.

### Editors

```bash
cargo install mq-content-lint --features lsp
```

`mq-content-lint-lsp` is a [Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
server, so any LSP-capable editor can get live diagnostics, hover text, and quick-fix code actions
by pointing it at the binary over stdio — it reuses the library directly rather than shelling out
to the CLI and parsing its output. Editing an `mq-content-lint.toml` re-lints every open document
automatically, no server restart needed, for clients that support watched-file notifications
(VS Code does out of the box). A [VS Code extension](./editors/vscode) built on it ships in this
repo; it isn't on the Marketplace yet, so see that directory's README for running it from source.
Other editors (Neovim, Helix, Zed, ...) can wire it up with their usual generic-LSP configuration,
pointing at `mq-content-lint-lsp` for Markdown files.

## Configuration

Drop a `mq-content-lint.toml` file in (or above) the directory you run `mq-content-lint` from —
it's discovered automatically, the same way `.eslintrc`/`pyproject.toml` are, by walking up from
the current directory. Pass `--config path/to/file.toml` to use an explicit path instead.

Config files **cascade** like ESLint's: every `mq-content-lint.toml` found from the current
directory up to the filesystem root is loaded, not just the nearest one. They're layered
farthest-first, so a closer file's `[rules]` entries win over the same key from a farther one —
put shared defaults at a monorepo's root and narrow them per-package. `front_matter.required_keys`
is inherited from the nearest ancestor that sets any (an empty/unset one in a closer file doesn't
clear an inherited value); `custom_rules` accumulate across every level instead of overriding,
since they're typically additive checks rather than competing settings.

A rule-specific key inside `[rules.<id>]` is validated against that rule's known options — a typo
like `[rules.line_length] limt = 100` is a config error at load time, not a silently-ignored key.

```toml
[rules]
# A rule accepts a bool (enable/disable at its default severity), a severity string ("error",
# "warning", "info", which also implies the rule is enabled), or a table for rules with extra
# options (a severity override plus whatever keys that rule reads — see each rule's row below).
heading_hierarchy_skip = "warning"
image_missing_alt = "error"
no_inline_html = false

[rules.line_length]
severity = "warning"
limit = 100
code_blocks = false

[rules.heading_style]
style = "atx"

[front_matter]
# Keys required in every linted document's YAML (`---`) or TOML (`+++`) front matter block.
required_keys = ["title"]
```

See [`mq-content-lint.toml`](./mq-content-lint.toml) in this repo for a fully-commented example.

**With no config file at all**, every rule runs at its default severity *except* the handful that
are opt-in by nature — `missing_front_matter_key` (no keys to require), `required_headings` (no
structure to require), `proper_names` (no names configured), and `link_image_style` (every style
allowed until you disallow one). Each has no universally sensible default, so "don't check it" is
the deterministic no-config behavior for those, not "silently guess one." Rule ids, default
severities, and mq selectors are listed below and are stable across releases within a major
version, as are diagnostic positions and JSON/SARIF field names — safe to depend on in CI.

## Built-in rules

Rule ids are this crate's own `snake_case` names (config keys and `--disable`/`--list-rules`
values), cross-referenced against their [markdownlint](https://github.com/DavidAnson/markdownlint)
equivalent for readers coming from that tool. "Fix?" marks whether the rule ever populates a
machine-applicable rewrite (see [Autofix](#autofix)). Want a check that isn't here? A
project-specific one is usually a better fit as a [custom rule](#custom-rules) than a PR; see
[CONTRIBUTING.md](./CONTRIBUTING.md) for adding a new built-in rule.

### Headings

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `heading_hierarchy_skip` | MD001 | warning | | Heading depth jumps by more than one level (`#` → `###`). |
| `heading_style` | MD003 | warning | ✓ | ATX / closed-ATX / setext style consistency (`[rules.heading_style] style`). |
| `no_missing_space_atx` | MD018 | warning | ✓ | `#Title` missing the space after `#`. |
| `no_multiple_space_atx` | MD019 | warning | ✓ | Multiple spaces after `#`. |
| `no_missing_space_closed_atx` | MD020 | warning | ✓ | `# Title#` missing the space before the closing `#`. |
| `no_multiple_space_closed_atx` | MD021 | warning | ✓ | Multiple spaces before the closing `#`. |
| `blanks_around_headings` | MD022 | warning | ✓ | Heading not surrounded by blank lines. |
| `heading_start_left` | MD023 | warning | ✓ | Heading indented instead of starting at column 1. |
| `no_duplicate_heading` | MD024 | warning | | Two headings with identical text. |
| `single_h1` | MD025 | warning | | More than one top-level (`h1`) heading. |
| `no_trailing_punctuation_heading` | MD026 | warning | ✓ | Heading ends in `.,;:!` (configurable, `?` excluded by default). |
| `no_emphasis_as_heading` | MD036 | info | | A whole line of `**bold**`/`*italic*` that looks like an intended heading. |
| `first_line_heading` | MD041 | info | | Document doesn't start with an `h1` (after front matter). |
| `required_headings` | MD043 | warning | | Heading structure doesn't match a configured sequence (`*` wildcard supported). |

### Lists

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `ul_style` | MD004 | warning | ✓ | Unordered marker (`-`/`*`/`+`) consistency. |
| `list_indent` | MD005 | warning | ✓ | Sibling list items at the same level indented inconsistently. |
| `ul_indent` | MD007 | warning | ✓ | Unordered sub-list indented by other than the configured width (default 2). |
| `ol_prefix` | MD029 | warning | ✓ | Ordered list numbering isn't all-same or strictly sequential. |
| `list_marker_space` | MD030 | warning | ✓ | Spaces after a list marker other than the configured count (default 1). |
| `blanks_around_lists` | MD032 | warning | ✓ | A list block not surrounded by blank lines. |

### Whitespace

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `no_trailing_spaces` | MD009 | warning | ✓ | Trailing whitespace (exactly 2, a hard break, allowed by default). |
| `no_hard_tabs` | MD010 | warning | ✓ | Hard tab characters. |
| `no_multiple_blanks` | MD012 | warning | ✓ | More than one consecutive blank line. |
| `line_length` | MD013 | info | | Line longer than the configured limit (default 80). |
| `no_multiple_space_blockquote` | MD027 | warning | ✓ | Multiple spaces after blockquote `>`. |
| `no_blanks_blockquote` | MD028 | warning | | Blank line inside a blockquote. |
| `single_trailing_newline` | MD047 | warning | ✓ | File doesn't end with exactly one newline. |

### Code blocks

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `fenced_code_language` | MD040 | info | | Fenced code block with no language specified. |
| `code_block_style` | MD046 | warning | | Fenced vs. indented style consistency. |
| `code_fence_style` | MD048 | warning | | Fence character (`` ` `` vs `~`) consistency. |
| `blanks_around_fences` | MD031 | warning | ✓ | A fenced code block not surrounded by blank lines. |
| `commands_show_output` | MD014 | info | ✓ | Every line in a block is a `$ command` with no output shown. |

### Links and images

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `image_missing_alt` | MD045 | error | | Image/image-ref with empty alt text. |
| `no_bare_urls` | MD034 | warning | ✓ | `https://...` not wrapped in `<>` or link syntax. |
| `no_reversed_links` | MD011 | warning | ✓ | `(text)[url]` instead of `[text](url)`. |
| `no_empty_links` | MD042 | warning | | Link with no real destination (empty or `#`). |
| `reference_links_images` | MD052 | error | | `[text][label]`/`[text][]` with no matching definition. |
| `link_image_reference_definitions` | MD053 | info | ✓ | A `[label]: url` definition nothing references. |
| `link_image_style` | MD054 | info | | Disallowed link/image style (opt-in per style, see config). |
| `link_fragments` | MD051 | warning | | `#fragment` link not matching any heading's slug. |
| `descriptive_link_text` | MD059 | info | | Generic link text ("click here", configurable). |
| `no_space_in_links` | MD039 | warning | ✓ | Spaces just inside `[ text ]`. |

### Inline formatting

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `no_space_in_emphasis` | MD037 | warning | ✓ | Spaces just inside `* text *`. |
| `no_space_in_code` | MD038 | warning | ✓ | Spaces just inside `` ` text ` ``. |
| `emphasis_style` | MD049 | warning | ✓ | `*text*` vs `_text_` consistency. |
| `strong_style` | MD050 | warning | ✓ | `**text**` vs `__text__` consistency. |
| `no_inline_html` | MD033 | warning | | Raw HTML (configurable allow-list). |
| `proper_names` | MD044 | warning | ✓ | Configured proper names in the wrong case. |
| `hr_style` | MD035 | warning | ✓ | Horizontal rule (`---`/`***`/`___`) consistency. |

### Tables

| Rule ID | MD | Severity | Fix? | Checks |
|---|---|---|:-:|---|
| `table_pipe_style` | MD055 | warning | ✓ | Leading/trailing `\|` consistency. |
| `table_column_count` | MD056 | warning | | A row with more/fewer cells than the header. |
| `blanks_around_tables` | MD058 | warning | ✓ | A table not surrounded by blank lines. |

### Front matter

| Rule ID | Severity | Fix? | Checks |
|---|---|:-:|---|
| `missing_front_matter_key` | error | | Required YAML/TOML front matter key missing (not a markdownlint rule). |

## Custom rules

Beyond the built-in rules, `mq-content-lint` lets a config file define its own rules as
[mq](https://github.com/harehare/mq) queries — this is the feature that sets it apart from
markdownlint/rumdl, which only ship fixed rule sets. A custom rule's query runs against the
document with `mq-lang`; every node it selects becomes a diagnostic at that node's position.

```toml
[[custom_rules]]
id = "no_todo"
query = 'select(contains(to_text(), "TODO"))'
message = "found a TODO marker"
severity = "warning"              # optional, defaults to "warning"
fix = 'replace("TODO", "DONE")'   # optional — see below
```

Custom rules run alongside the built-ins and are merged into the same report, sorted by position.
Their `ruleId` is whatever `id` you configure (not one of the built-in ids below), their
`selector` field in JSON output is always `null`, and an invalid query is a hard error at lint
time (not a silently-empty result), so a typo in a query fails loudly rather than passing CI by
accident.

An optional `fix` expression makes a custom rule autofixable: for each node `query` matches, `fix`
runs with that single node as input, and its result (stringified) replaces the node's full span
under `--fix`/`--diff` — the same mechanism a built-in rule's fix uses. Omit `fix` for a
report-only rule.

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.

## License

MIT
