# Contributing

## Two ways to add a rule

| | Built-in rule (`src/rules/*.rs`) | Custom rule (`[[custom_rules]]`) |
|---|---|---|
| Where it lives | This repo, compiled into the binary | The *user's* `mq-content-lint.toml` |
| Language | Rust, against the `mq-markdown` AST | An `mq` query string |
| `--fix` support | Can populate a [`Fix`](src/fix.rs) | Never — report-only |
| Ships to everyone | Yes, on the next release | No — project-specific by nature |
| Use it for | A check general enough that other users of this tool would want it too | Something specific to one project/org (a house style rule, a required disclaimer, "no TODO left in shipped docs") |

If you just want to check for something in *your* docs, reach for a custom rule first — see the
[README's Custom rules section](./README.md#custom-rules). No code change, no release wait. The
rest of this document is about adding a **built-in** rule, which is the right call when the check
is broadly useful (most of markdownlint's `MDxxx` rules fall here) or needs an autofix.

## Adding a built-in rule

Every built-in rule touches the same five places. Skipping one fails a test (`all_rules_matches_rule_id_all`,
`rule_ids_and_selectors_are_stable`, or the `RuleId::ALL`/`as_str`/`selector` exhaustiveness match
arms simply won't compile), so the compiler and test suite catch a half-finished rule for you —
follow the errors if you lose track of a step.

1. **`src/message.rs`** — add the rule's identity and message shape:
   - A `RuleId` variant, with a doc comment giving its markdownlint `MDxxx` cross-reference (or
     noting it has none, like `MissingFrontMatterKey`).
   - An entry in `RuleId::ALL` (order here is the stable, documented order everywhere else —
     `--list-rules`, README's rule table, `all_rules()` — so put it near related rules, not
     necessarily at the end).
   - An arm in `RuleId::as_str()` — the `snake_case` config key / `--disable` value / SARIF-JSON
     `ruleId`. This string is a stability guarantee once released; don't rename it later.
   - An arm in `RuleId::selector()` — the single `mq_lang::Selector` the rule's diagnostics center
     on (`Some(Selector::Heading(None))`, etc.), or `None` if the rule scans raw lines/spans
     multiple node types with no single selector (see `NoTrailingSpaces`, `LineLength`).
   - An arm in `RuleId::description()` — same text as the `RuleId` variant's doc comment (`--explain`
     reads this at runtime; doc comments aren't available there, so it's kept in sync by hand).
   - A `LintMessage` variant carrying whatever data the message needs to render (the offending
     value, the expected value, ...) — prefer this over a raw `String` so `Display`/`help()` stay
     exhaustive and typo-proof.
   - Arms in `LintMessage::rule_id()`, `LintMessage::help()` (the "how to fix this" hint shown
     alongside the diagnostic — write one even if the rule is unfixable by machine), and
     `LintMessage::Display` (the diagnostic text itself).

2. **`src/rules/<rule_name>.rs`** — the rule's logic, implementing the
   [`Rule`](src/rules.rs) trait:
   ```rust
   impl Rule for MyRule {
       fn id(&self) -> RuleId { RuleId::MyRule }
       fn default_severity(&self) -> Severity { Severity::Warning }
       fn check(&self, doc: &mq_markdown::Markdown, source: &str, config: &LintConfig) -> Vec<Diagnostic> {
           // ...
       }
   }
   ```
   Module-level doc comment: state the `MDxxx` equivalent and one line on what triggers it (see
   any existing rule file for the pattern). A few things every rule author hits:
   - **AST or raw source?** Walk the AST (`crate::walk::walk`, or a plain `doc.nodes` loop for
     top-level-only checks) when node type/nesting is what matters. Drop to `source` via
     `crate::text::numbered_lines`/`LineIndex` when you need exact whitespace, marker characters,
     or indentation — mq-markdown's AST normalizes those away (e.g. a heading's
     `Position::start.column` is always 1 regardless of real indentation; see `heading_start_left.rs`
     for the raw-line workaround). GFM autolinking also means bare URLs may already be `Link`
     nodes, not `Text` — see `no_bare_urls.rs`'s doc comment for that trap.
   - **O(1) line lookup.** If you loop over matching nodes and need each one's source line, build
     one `LineIndex::new(source)` before the loop, not a `numbered_lines(source).find(...)` per
     node — the latter is O(nodes × lines), quadratic on a document where matches scale with size.
     Scanning raw lines and need to skip fenced code blocks (see "AST or raw source?" above)? Build
     one `CodeBlockLines::new(code_ranges)` before the line loop and call `.contains(line_number)`,
     not `code_ranges.iter().any(|(start, end)| ...)` per line — same shape of bug, O(lines × code
     blocks) instead of O(lines + log code blocks); see `no_bare_urls.rs` for the pattern.
   - **Fix or no fix.** Populate `.with_fix(Fix::new(range, replacement))` when there's exactly one
     correct mechanical rewrite. Leave it off when the rule can only report (no sensible default
     alt text, no way to invent front matter content, an opening/closing pair on different lines —
     `Fix` is a single-range replacement, so a two-line change isn't expressible as one). If the
     rule *never* populates a fix, override `fn fixable(&self) -> bool { false }` too (defaults to
     `true`) — it's what `--list-rules`/`--explain` show, and what the README's rule tables should
     agree with.
   - **Config.** Read rule-specific options via `config.rule_options(self.id())`'s `get_bool`/
     `get_usize`/`get_str`/`get_str_array`, falling back to a hardcoded default — see
     `no_hard_tabs.rs`'s `spaces` option for the minimal pattern. Don't consult severity from
     `config` yourself; `Linter::run` applies the configured override after `check()` returns.
   - Inline `#[cfg(test)] mod tests` covering the no-op case, the firing case, and (if
     configurable) a non-default-config case. For a rule complex enough to want realistic
     multi-paragraph fixtures, add `tests/fixtures/<rule_name>/{ok,bad,not_autofixable}.md` and a
     block in `tests/lint_test.rs` following the existing three rules there.

3. **`src/rules.rs`** — add `mod <rule_name>;` and `Box::new(<rule_name>::MyRule)` to
   `all_rules()`, at the same relative position as its `RuleId::ALL` entry (the
   `all_rules_matches_rule_id_all` test enforces the two lists stay in lockstep).

4. **`README.md`** — add a row to the relevant category table under **Built-in rules** (Rule ID,
   `MDxxx`, default severity, whether it has a fix, one-line description). Check the new row's
   own line length and the diagnostic count (`mq-content-lint README.md`) before and after —
   README's rule tables are the one place this crate documents its own bad-pattern examples
   (`` `(text)[url]` ``, a literal `TODO`, ...) in backticks, so a rule whose selector matches
   plain text can fire on its own documentation. **Never run `--fix`/`--diff` against
   `README.md`** to "clean up" anything it flags — every rule capable of firing on that page has
   already been checked against it once; a fix pass will happily rewrite an illustrative example
   into nonsense (a real incident: an early draft of MD060 auto-"fixed" the `no_reversed_links`
   row's own `` `(text)[url]` `` example into `` `[text](url)` ``, and the `no_todo` custom rule in
   this repo's own `mq-content-lint.toml` turned a `TODO`-detection example into `DONE`). If a new
   rule flags README lines that turn out to need a real edit, make that edit by hand and re-run
   `mq-content-lint README.md` to confirm the diagnostic is gone before moving on — if it isn't
   real, it usually means the rule's design needs a second look (see the compact-style empty-cell
   case in `table_column_style.rs`'s history), not that the README needs to change.

5. Run the full check before opening a PR:
   ```sh
   cargo test --all-features --locked
   cargo clippy --all-targets --all-features --locked -- -D warnings
   cargo fmt --all -- --check
   ```

## Project conventions

- Rule ids are `snake_case` and, once released, are load-bearing strings (config keys, CLI flags,
  SARIF `ruleId`) — treat a rename as a breaking change, not a refactor.
- Diagnostics are sorted by `(start_line, start_column)` before being returned from `Linter::run`;
  individual rules don't need to sort their own output.
- Keep `RuleId`/`LintMessage` closed enums — they're deliberately not extensible from outside the
  crate (that's why custom rules are a wholly separate `CustomRule`/`CustomDiagnostic` type in
  `src/custom_rules.rs` rather than a `RuleId` variant).
