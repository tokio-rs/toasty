# Toasty User Guide

Source for the Toasty user guide, rendered with
[mdBook](https://rust-lang.github.io/mdBook/). The published version
lives at <https://tokio-rs.github.io/toasty/nightly/guide/>.

## Layout

```
docs/guide/
├── book.toml                 mdBook configuration
├── mdbook-toasty-version     preprocessor (Python) for {{toasty_version}}
├── src/                      chapter sources (Markdown)
│   ├── SUMMARY.md            table of contents — order shown in the sidebar
│   └── *.md                  one file per chapter
└── book/                     build output (gitignored)
```

Chapters are auto-discovered from `SUMMARY.md`. Adding a new chapter
means creating `src/<name>.md` and linking it from `SUMMARY.md`.

## Tooling

The CI workflow at [`.github/workflows/docs.yml`](../../.github/workflows/docs.yml)
pins mdBook to the version below. Match it locally so local builds and
CI behave the same:

```sh
cargo install mdbook@0.5.2 mdbook-linkcheck2
```

The `linkcheck2` backend is marked `optional = true` in `book.toml`,
so a missing or broken linkcheck installation does not block local
builds.

The `mdbook-toasty-version` preprocessor is a Python 3 script invoked
by mdBook; it needs no install.

## Working on the guide

### Live preview

```sh
mdbook serve docs/guide --open
```

Opens the rendered guide in a browser and rebuilds on every save.
This is the right loop for iterating on a chapter.

### One-shot build

```sh
mdbook build docs/guide
```

Writes HTML to `docs/guide/book/`. CI runs this exact command.

### Test code samples

```sh
cargo test -p tests --doc
```

The guide's runnable examples are not tested via `mdbook test` (which
would have no way to find the `toasty` crate). Instead,
[`tests/build.rs`](../../tests/build.rs) auto-discovers every `.md`
file in `src/` and emits `#[doc = include_str!(...)]` modules so
rustdoc compiles each fenced ```rust block with the full toasty
dependency graph available. Blocks marked `rust,ignore`, `rust,no_run`,
or `compile_fail` are skipped or handled per the standard rustdoc
rules.

Use hidden `# `-prefixed boilerplate (imports, model definitions,
async wrapper) to keep example bodies focused — the same convention
as Rust API doc comments.

## The `{{toasty_version}}` placeholder

Chapters can reference the current Toasty version with the literal
string `{{toasty_version}}`. The preprocessor reads the version from
`crates/toasty/Cargo.toml` at build time and substitutes the
`major.minor` form, so a `Cargo.toml` example like

````md
```toml
toasty = { version = "{{toasty_version}}", features = ["sqlite"] }
```
````

renders with the version that's actually published.

## Writing style

Follow the conventions in the
[`prose`](../../.claude/skills/prose/SKILL.md) skill: fact-focused,
direct, present tense, active voice. No buzzwords or business jargon.
Show with concrete examples rather than abstract description.

## Keeping the guide current

Toasty iterates quickly. The
[`sync-docs`](../../.claude/skills/sync-docs/SKILL.md) skill walks
the commit log for user-observable changes and updates this guide and
the rustdoc to match. Run it periodically — or before a release —
to catch features that have landed without doc updates.
