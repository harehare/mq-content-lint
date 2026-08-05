# mq-content-lint

Static content linter for Markdown, built on [mq](https://github.com/harehare/mq)'s
`mq-markdown` AST — the same document model mq's own query engine and `mq-lint` use, reused here
instead of hand-rolling another Markdown parser.

`mq-content-lint` checks the *content* of a document — heading structure, image accessibility,
required front matter — the territory markdownlint's structural rules and Vale's shareable
styles cover, expressed against mq's node model instead of a bespoke rule engine. It is a
separate tool from [`mq-lint`](https://github.com/harehare/mq/tree/main/crates/mq-lint), which
lints `.mq` query scripts, not Markdown content, and from user-supplied mq expressions as rules,
which is a later stage of this project (see [Non-goals](#non-goals)).

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

# Machine-readable output
mq-content-lint --format json docs/ > report.json
mq-content-lint --format sarif docs/ > report.sarif

# List built-in rules, their default severity, and the mq selector each corresponds to
mq-content-lint --list-rules
```

Exit code is non-zero if any diagnostic at or above `--min-severity` (default `info`, i.e. "any
diagnostic at all") was reported — the same convention `mq-lint` uses, so both tools fail CI the
same way. Pass `--min-severity error` to only fail on errors.

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
# Each rule accepts either a bool (enable/disable at its default severity) or a severity
# string ("error", "warning", "info"), which also implies the rule is enabled.
heading_hierarchy_skip = "warning"
image_missing_alt = "error"
missing_front_matter_key = "error"

[front_matter]
# Keys required in every linted document's YAML (`---`) or TOML (`+++`) front matter block.
required_keys = ["title"]
```

See [`mq-content-lint.toml`](./mq-content-lint.toml) in this repo for a fully-commented example.

**With no config file at all**, every rule runs at its default severity *except*
`missing_front_matter_key`, which has no required keys to check and so never fires — there's no
universally sensible key to require by default, so "don't check front matter keys" is the
deterministic no-config behavior, not "silently guess one." Rule ids, default severities, and mq
selectors are listed below and are stable across releases within a major version, as are
diagnostic positions and JSON/SARIF field names — safe to depend on in CI.

## Built-in rules

| Rule ID                     | Default severity | mq selector | Checks |
|------------------------------|-------------------|-------------|--------|
| `heading_hierarchy_skip`     | `warning`         | `.h`        | A heading's depth jumps by more than one level from the previous heading (`#` directly followed by `###`), equivalent to markdownlint's MD001. Decreasing depth is always fine; the document's first heading is never flagged regardless of its level. |
| `image_missing_alt`          | `error`           | `.image`    | An image or image reference (`![alt](url)` / `![alt][ref]`) has empty alt text — the same accessibility check as mq's own cookbook query [`select(.image.alt == "")`](https://github.com/harehare/mq/blob/main/docs/books/src/cookbook/find-images-missing-alt-text.md), run here as a built-in rule. |
| `missing_front_matter_key`   | `error`           | `.yaml`     | The document's YAML/TOML front matter is missing a key listed in `front_matter.required_keys`. Also flags a document with no front matter at all (once) if any keys are required, and front matter that fails to parse as YAML/TOML. |

None of these rules can be applied automatically — there's no reasonable default alt text, no
single correct place to insert a skipped heading level, and no way to invent the value of a
missing front matter key — so, unlike `mq-lint`, `mq-content-lint` has no `--fix` flag and no
diagnostic carries a machine-applicable rewrite. Every finding is one for a human to resolve; see
`tests/fixtures/*/not_autofixable.md` for a concrete example per rule.

## Non-goals

- **Natural-language spelling/style checking** (a Vale-style prose linter) is out of scope.
- **Arbitrary mq expressions as rules** — letting a config file supply its own `.mq` query as a
  custom rule — is a later stage of this project. The three rules above are fixed, built-in
  Rust logic over the `mq-markdown` AST, not user-supplied queries, which is what makes their
  output deterministic and their positions stable enough to depend on in CI today.

## License

MIT
