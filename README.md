# mq-content-lint

> [!NOTE]
> This project is developed entirely on [Claude Code](https://claude.ai/code) for Android — no
> desktop or laptop development environment is used.

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
# Write a starter mq-content-lint.toml (everything commented out — see Configuration)
mq-content-lint --init

# Lint one file
mq-content-lint README.md

# Lint a directory recursively (.md / .markdown files; dotfiles/dotdirs, node_modules, and
# target are always skipped, along with anything .gitignore'd, .mq-content-lintignore'd,
# or matched by the config's `ignore` patterns — see Configuration)
mq-content-lint docs/

# Read from stdin (running with no arguments and no piped input prints a hint on stderr instead
# of just hanging, in case you meant to pass a file/directory)
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
mq-content-lint --format rdjson docs/ > report.rdjson  # for reviewdog — see GitHub Actions

# List built-in rules, their default severity, and the mq selector each corresponds to
mq-content-lint --list-rules

# Print a rule's description, markdownlint equivalent, severity, selector, and options
mq-content-lint --explain line_length

# Print the installed version
mq-content-lint --version

# Print a shell completion script (bash, zsh, fish, powershell, or elvish)
mq-content-lint --generate-completions zsh > ~/.zsh/completions/_mq-content-lint

# Print a roff man page
mq-content-lint --generate-man-page > /usr/local/share/man/man1/mq-content-lint.1

# Print a JSON Schema for mq-content-lint.toml — see Configuration
mq-content-lint --print-json-schema > mq-content-lint.schema.json
```

Exit code is non-zero if any diagnostic at or above `--min-severity` (default `info`, i.e. "any
diagnostic at all") was reported — the same convention `mq-lint` uses, so both tools fail CI the
same way. Pass `--min-severity error` to only fail on errors.

When given more than one file, they're read, (optionally) fixed, and linted in parallel across
CPU cores; output stays in the same order regardless (sorted by path), so multi-file runs are
deterministic to diff or snapshot in CI.

### Autofix

`--fix` applies every diagnostic with a machine-applicable rewrite, then re-lints and re-fixes
automatically if that exposed a new issue — e.g. fixing two tight, back-to-back headings' missing
`#` spacing can leave them separated by two blank lines instead of one, which a second pass then
collapses — repeating up to 10 times (the same convention ESLint's own `--fix` uses) until a pass
makes no further change. Rules where there's no single unambiguous rewrite — no reasonable default
alt text, no way to invent required front matter content, an ordered-list prefix bug that could
mean either "insert here" or "renumber" — never populate a fix; see the "Fix?" column below (or
`--explain <rule-id>`'s `fixable:` line) for which. A [custom rule](#custom-rules) is fixable too,
if it's configured with a `fix` expression.

Not sure what `--fix` would do before it does it? `--diff` computes the exact same fixes but never
writes — files (and stdin's fixed content) stay untouched, and a unified diff (colored like `git
diff` — removed lines red, added lines green — when writing to a terminal) is printed to stdout
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

Each SARIF result's rule declaration carries a `shortDescription` (the same text `--explain`
prints), so GitHub's code scanning UI shows what a finding checks, not just its bare rule id.

Prefer inline PR review comments over a code-scanning report? Use `format: rdjson` with
[reviewdog](https://github.com/reviewdog/reviewdog) — a diagnostic with a fix comes through as a
[suggested change](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/incorporating-feedback-in-your-pull-request)
reviewers can apply with one click:

```yaml
- uses: harehare/mq-content-lint@v1
  id: lint
  with:
    path: docs/
    format: rdjson
  continue-on-error: true
- uses: reviewdog/action-setup@v1
- run: |
    reviewdog -f=rdjson -reporter=github-pr-review < "${{ steps.lint.outputs.rdjson-file }}"
  env:
    REVIEWDOG_GITHUB_API_TOKEN: ${{ secrets.GITHUB_TOKEN }}
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
to the CLI and parsing its output. A burst of rapid edits (fast typing, a large paste) is debounced
into a single re-lint ~200ms after things go quiet, rather than a full re-lint on every keystroke.
Every diagnostic with a machine-applicable fix offers it as a
quick fix; every diagnostic also offers a second quick fix that inserts a [disable
comment](#inline-disable-comments) for just that line, for the cases a fix can't cover. Editing an
`mq-content-lint.toml` re-lints every open document automatically, no server restart needed, for
clients that support watched-file notifications (VS Code does out of the box). A [VS Code
extension](./editors/vscode) built on it ships in this repo; it isn't on the Marketplace yet, so
see that directory's README for running it from source. Other editors (Neovim, Helix, Zed, ...) can
wire it up with their usual generic-LSP configuration, pointing at `mq-content-lint-lsp` for
Markdown files.

## Configuration

Drop a `mq-content-lint.toml` file in (or above) the directory you run `mq-content-lint` from —
it's discovered automatically, the same way `.eslintrc`/`pyproject.toml` are, by walking up from
the current directory. Pass `--config path/to/file.toml` to use an explicit path instead.

`mq-content-lint --init` writes a starter one in the current directory to save typing it from
scratch — every setting in it is commented out, so it documents what's available without changing
any rule's behavior until you uncomment something. Refuses to overwrite an existing config.

Config files **cascade** like ESLint's: every `mq-content-lint.toml` found from the current
directory up to the filesystem root is loaded, not just the nearest one. They're layered
farthest-first, so a closer file's `[rules]` entries win over the same key from a farther one —
put shared defaults at a monorepo's root and narrow them per-package. `front_matter.required_keys`
is inherited from the nearest ancestor that sets any (an empty/unset one in a closer file doesn't
clear an inherited value); `custom_rules` and `ignore` accumulate across every level instead of
overriding, since they're typically additive rather than competing settings.

A rule-specific key inside `[rules.<id>]` is validated against that rule's known options — a typo
like `[rules.line_length] limt = 100` is a config error at load time, not a silently-ignored key.

A typo'd rule name — `[rules] line_lenght = true`, or `--explain`/`--disable <rule-id>` on the
command line — gets a "did you mean `line_length`?" suggestion in the error when one built-in rule
id is a close enough match.

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

### Editor autocomplete and validation

`mq-content-lint --print-json-schema > mq-content-lint.schema.json` writes a JSON Schema for
`mq-content-lint.toml` — rule names, severity strings, and the `front_matter`/`custom_rules`/
`ignore` shapes, so an editor can flag a typo'd rule name or option before you ever run the
linter. Reference it from the top of the config file with a `#:schema` pragma comment (supported
by [Taplo](https://taplo.tamasfe.dev/) / the "Even Better TOML" VS Code extension):

```toml
#:schema ./mq-content-lint.schema.json

[rules]
...
```

Regenerate the schema after upgrading if new rules were added — it isn't published anywhere, so
each project keeps its own local copy.

### `.editorconfig`

If a project has an [`.editorconfig`](https://editorconfig.org) with `max_line_length` set,
`line_length`'s `limit` falls back to it when `mq-content-lint.toml` doesn't set one explicitly (a
config file's own `limit` always wins). No other `.editorconfig` property is read — properties
like `indent_size` don't map cleanly onto `ul_indent`/`list_indent`, which count spaces per list
nesting level rather than a single document-wide indent width, so this crate doesn't guess at a
mapping for them.

**With no config file at all**, every rule runs at its default severity *except* the handful that
are opt-in by nature — `missing_front_matter_key` (no keys to require), `required_headings` (no
structure to require), `proper_names` (no names configured), and `link_image_style` (every style
allowed until you disallow one). Each has no universally sensible default, so "don't check it" is
the deterministic no-config behavior for those, not "silently guess one." Rule ids, default
severities, and mq selectors are listed below and are stable across releases within a major
version, as are diagnostic positions and JSON/SARIF field names — safe to depend on in CI.

### Ignoring files

When linting a directory, `mq-content-lint` skips `node_modules`/`target` unconditionally, plus
dotfiles/dotdirs, plus anything excluded by a `.gitignore`, `.git/info/exclude`, or global
gitignore it finds along the way (only inside an actual git repository — same rule ripgrep and
most other modern CLI tools follow). Two more ways to exclude paths, both gitignore-syntax:

- A `.mq-content-lintignore` file, anywhere in the tree being walked — for exclusions a project
  doesn't want (or can't put, if the file itself is tracked) in `.gitignore`.
- An `ignore` array in `mq-content-lint.toml`:

  ```toml
  ignore = ["vendor/**", "CHANGELOG.md", "!vendor/keep-this.md"]
  ```

  (a leading `!` re-includes a path an earlier pattern excluded, same as `.gitignore`). This
  accumulates across cascaded config files rather than overriding, like `custom_rules` does.

None of this applies to a file named directly on the command line — `mq-content-lint
vendor/lib.md` always lints it, ignore patterns or not, the same way `git add <path>` does.

### Inline disable comments

A single false positive doesn't need a config change — drop an HTML comment where the problem is:

```markdown
<!-- mq-content-lint-disable line_length -->
This line can be as long as it wants to be now, no matter what `[rules.line_length]` says.
<!-- mq-content-lint-enable line_length -->

<!-- mq-content-lint-disable-next-line no_bare_urls -->
See https://example.com for details.
```

Four directives, each its own HTML comment on its own line (a comment sharing a line with other
text isn't recognized, on purpose — keeps the syntax unambiguous to spot):

| Directive | Effect |
| --- | --- |
| `<!-- mq-content-lint-disable [id, ...] -->` | Suppress the named rules (or every rule, with no ids) from this line onward. |
| `<!-- mq-content-lint-enable [id, ...] -->` | Re-enable the named rules (or every rule). |
| `<!-- mq-content-lint-disable-line [id, ...] -->` | Suppress only on the comment's own line. |
| `<!-- mq-content-lint-disable-next-line [id, ...] -->` | Suppress only on the following line. |

Rule ids are comma-separated and work identically for built-in rules and [custom
rules](#custom-rules) — matched against whatever id the diagnostic reports. Applied uniformly by
the CLI, `--fix`/`--diff`, and the LSP server, since they all funnel through the same lint entry
point.

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
| `table_column_style` | MD060 | warning | ✓ | Padding around `\|` (see `--explain`). |

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

### Multi-line queries

`query`/`fix` are plain TOML strings, so a query too long or complex for one line can use TOML's
triple-quoted syntax — mq doesn't treat newlines as meaningful, so this is purely a readability
choice:

```toml
[[custom_rules]]
id = "no_todo"
query = '''
select(
  contains(to_text(), "TODO")
)
'''
message = "found a TODO marker"
```

### Document-wide rules

By default, `query` runs once per top-level node — fine for "does this node match a pattern," but
not for a check that depends on the whole document (how many of something there are, whether two
distant headings collide). Start the query with mq's `nodes` keyword to gather every top-level
node into one array first, then the rest of the pipeline runs once against that array instead of
once per node:

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

This flags every `h1` in the document, but only when there's more than one — the same check the
`single_h1` built-in makes, expressible as a custom rule because it needs to see every heading at
once to count them. Two things that trip people up the first time:

- A bare selector like `.h1` applied to the whole-document array (i.e. after `nodes`) is a *map*,
  not a *filter* — it keeps the array's length, replacing each non-matching element with `none`
  rather than dropping it. `len(.h1)` therefore counts every top-level node, not just the
  matching ones; wrap it in `compact()` first (`len(compact(.h1))`) to drop the `none`s and get
  the real count. mq-content-lint's own per-node mode doesn't have this gotcha — it relies on
  exactly this same "non-matches become `none`" behavior to mean "no diagnostic here," which is
  invisible until you try to `len()` the result yourself.
- `nodes` is only valid once, as the very first step of the top-level pipeline — not nested inside
  a `let`, an `if`, or a function call's arguments (a second `nodes` there is a syntax error, not a
  second whole-document view).

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.

## License

MIT
