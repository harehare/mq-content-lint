# mq-content-lint

Static content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s
`mq-markdown` AST — the same document model mq's own query engine and `mq-lint` use, reused here
instead of hand-rolling another Markdown parser.

`mq-content-lint` checks the *content* of a document — heading structure, list and table
consistency, whitespace, link/image hygiene, required front matter — comprehensive coverage of
[markdownlint](https://github.com/DavidAnson/markdownlint)'s rule set, expressed against mq's
node model instead of a bespoke rule engine. It is a separate tool from
[`mq-lint`](https://github.com/harehare/mq/tree/main/crates/mq-lint), which lints `.mq` query
scripts, not Markdown content, and from user-supplied mq expressions as rules, which is a later
stage of this project (see [Non-goals](#non-goals)).

## Install

```bash
cargo install mq-content-lint
```

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
"Fix?" column below.

### GitHub Actions

```yaml
- name: Install mq-content-lint
  run: cargo install mq-content-lint
- name: Lint docs
  run: mq-content-lint --format sarif docs/ > mq-content-lint.sarif
  continue-on-error: true
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: mq-content-lint.sarif
```

## Configuration

Drop a `mq-content-lint.toml` file in (or above) the directory you run `mq-content-lint` from —
it's discovered automatically, the same way `.eslintrc`/`pyproject.toml` are, by walking up from
the current directory. Pass `--config path/to/file.toml` to use an explicit path instead.

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
severity = "warning"  # optional, defaults to "warning"
```

Custom rules run alongside the built-ins and are merged into the same report, sorted by position.
They don't currently support `--fix` — a custom rule only reports, it doesn't rewrite. Their
`ruleId` is whatever `id` you configure (not one of the built-in ids below), their `selector`
field in JSON output is always `null`, and an invalid query is a hard error at lint time (not a
silently-empty result), so a typo in a query fails loudly rather than passing CI by accident.

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.

## License

MIT
