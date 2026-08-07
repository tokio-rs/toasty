# Standalone `toasty` CLI

## Summary

Ship `toasty-cli` as a single binary that users install with
`cargo install toasty-cli` and run from any Cargo package. The CLI extracts
the user's resolved schema by building the user's existing target — a bin or,
for lib-only crates, the lib as a `cdylib` — and running it with an env var
that triggers a constructor inside `toasty` to dump the schema and exit. No
per-project boilerplate, no synthesized companion crate, no manifest
mutation.

## Motivation

Previously, using Toasty's migration tooling required writing a per-project
CLI binary that links the user's models and dispatches to `toasty-cli` as a
library. That is friction for every new Toasty project and does not scale to
a workflow where `cargo install toasty-cli` should be enough.

[#824] makes the CLI standalone by synthesizing a sibling Cargo package
under `target/toasty-dump/` that path-depends on the user's lib, mirrors
their feature selection, and runs as a one-shot dumper binary. It works,
but it has rough edges:

- Lib-only is the only supported shape — bin-only crates are explicitly
  rejected.
- The synthesized manifest must mirror the user's `toasty` features, edition,
  and dep graph; drift means a different rlib is built and the dep cache is
  not reused.
- A second package under `target/` is one more thing for users to notice and
  for tooling to clean up.

A simpler scheme is available: instead of synthesizing a crate to host the
dumper, put the dumper *inside* `toasty` itself, gated on an env var. The
user's existing target becomes the dumper.

## User-facing API

Users install once:

```
cargo install toasty-cli
```

From a Cargo package that uses Toasty, run migration commands directly:

```
toasty migrate generate --flavor postgresql --name init
toasty migrate apply --url postgres://...
toasty migrate reset --url sqlite://app.db
```

`toasty migrate generate` and `toasty migrate snapshot` need the user's
schema. They compile the user's package and read the schema back out of the
build artifact. Because each flavor maps model types to different column
types, `generate` takes `--flavor` (or a `migration.flavor` default in
`Toasty.toml`). Other subcommands operate on saved migration files or talk
to a database directly via `--url` (defaulting to the `DATABASE_URL`
environment variable).

### Workspaces

In a workspace, the CLI uses the workspace root package by default. Use
`-p <pkg>` to select a different member, mirroring `cargo`:

```
toasty -p api migrate generate --flavor postgresql
```

`Toasty.toml` and the migration directory live in the selected package's
directory, so each member keeps its own migration history. If the workspace
has no root package (a virtual manifest) and `-p` is not supplied, the CLI
errors with the list of workspace members.

### What the user does not have to do

- No `Cargo.toml` changes in the user's package.
- No source changes — no `dump_if_requested()` call site, no `main.rs`
  edits.
- No mention of the schema-extraction mechanism in any user-visible
  configuration.

## Behavior

### Build target selection

Given a target package (root package, or `-p <pkg>`), the CLI picks an
artifact to build:

1. If the package has at least one `[[bin]]` target, build it with
   `cargo build --bin <name>`. When multiple bins exist,
   `toasty migrate generate --bin <name>` selects one explicitly; otherwise
   the CLI errors with the list of bin names.
2. Otherwise, if the package has a `[lib]` target, build it as a `cdylib`
   with `cargo rustc --crate-type cdylib`. This overrides the crate type
   without modifying `Cargo.toml`.
3. Otherwise, error: nothing to extract a schema from.

The build always runs in the `dev` profile. Release-profile concerns
(LTO, dead-stripping, link-section gc) do not apply to the schema-dump
ctor.

### The dump constructor

`toasty` itself contributes a constructor through [`linktime`]:

```rust
#[cfg(debug_assertions)]
#[linktime::ctor(unsafe)]
fn __toasty_maybe_dump_schema() {
    let Some(flavor) = std::env::var_os("TOASTY_DUMP_SCHEMA") else { return };
    // dump the schema for `flavor` to stdout, then
    std::process::exit(0);
}
```

This runs before `main` (for binaries) or during `dlopen` (for cdylibs).
The env var's value names the flavor (`sqlite`, `postgresql`, `mysql`,
`turso`). When it is set, the constructor collects the same `inventory`
registrations `#[derive(Model)]` already produces (sorted by model name, so
output is stable across rebuilds regardless of link order), builds an
`app::Schema`, lowers it with the flavor's `Capability`, serializes the
resulting `db::Schema` as JSON to stdout inside a versioned envelope, and
exits with status 0. An unknown flavor value prints an error listing the
valid names and exits with status 1. When the env var is not set, the
constructor returns immediately.

The dump payload is the `db::Schema`, not the `app::Schema`. The
`db::Schema` is what migration generation diffs and what snapshot files
already serialize, so it has a serde representation with a version field and
a compatibility expectation across releases. The `app::Schema` graph is not
serializable (`stmt::Value`, `stmt::Path`), and its IDs are assigned in
registration order, which would make it an unstable wire format between a
separately installed CLI and the user's `toasty` version. Lowering happens
in the user's process, where the user's `toasty-core` version is
authoritative; the CLI checks the envelope version and reports a version
mismatch instead of misparsing.

The ctor is gated on `cfg(debug_assertions)` so release builds carry no
schema-dump machinery at all.

Constructor preservation through `cargo rustc --crate-type cdylib` was
verified on Linux, macOS, and Windows during design review; the
implementation pulls in tests that exercise the same path on every CI
run.

### Running the bin path

For a bin target, the CLI invokes the artifact directly:

```
TOASTY_DUMP_SCHEMA=postgresql ./target/debug/<bin-name>
```

The ctor fires before `main` runs, dumps, and `exit(0)`s. The user's `main`
never executes. The CLI captures stdout and parses it.

### Running the cdylib path via re-exec

A `cdylib` has no entry point. Loading it with `dlopen` runs the
constructors in its initialization image, including the one contributed by
`toasty`. But `exit(0)` from inside a constructor would terminate the CLI
process itself, so the `dlopen` happens in a child.

Rather than ship a second binary, `toasty-cli` re-execs itself with a
hidden subcommand:

```
toasty __load-cdylib /path/to/libuser_app.dylib --flavor postgresql
```

The subcommand sets `TOASTY_DUMP_SCHEMA` in its own environment and then
loads the library with `libloading::Library::new`; the ctor dumps and exits
during the load. The flavor travels as an argument rather than as an env
var on the child because a debug build of the CLI links `toasty` itself and
would otherwise trigger its own dump constructor — with an empty schema —
before reaching the subcommand. The parent CLI captures the child's stdout
the same way it captures a bin's stdout. One binary is shipped; the re-exec
keeps the dump happening in a process the parent controls.

The `__load-cdylib` subcommand is hidden from `--help` and not part of
the public surface — its only caller is `toasty-cli` itself.

### End-to-end flow for `migrate generate`

1. `cargo metadata` to identify the target package and its layout.
2. Build the chosen artifact (`--bin <name>` or
   `cargo rustc --crate-type cdylib`), parsing
   `--message-format=json-render-diagnostics` to find the artifact path.
3. Invoke the dumper: spawn the bin with `TOASTY_DUMP_SCHEMA=<flavor>`, or
   re-exec `toasty __load-cdylib <artifact> --flavor <flavor>`.
4. Deserialize stdout as the versioned `db::Schema` envelope.
5. Diff against the latest snapshot, prompt for renames, write the
   migration and snapshot files.

## Edge cases

- **Virtual workspace root.** No root package; `-p` is required. The CLI
  reports the list of members.
- **Multiple bins in the target package.** The CLI requires `--bin <name>`
  unless exactly one bin is present.
- **Package with neither a bin nor a lib.** The CLI errors. Pure proc-macro
  or build-script-only packages are not supported.
- **Release-only build flags.** If the user has `[profile.dev] opt-level`
  or LTO settings, the ctor still runs — `linktime` uses `#[used]` plus
  link-section attributes that survive ordinary optimization. Aggressive
  cross-crate LTO at `dev` level is unusual; if a setting strips the ctor,
  the CLI errors with "the schema dumper produced no schema; check that
  `toasty` is a direct dependency of `<pkg>`."
- **`toasty` not actually depended on.** The ctor is in `toasty`; without
  the dependency the env var has no effect. The CLI detects this in
  `cargo metadata` and errors before building.
- **Release builds.** The ctor is `cfg(debug_assertions)`-gated, so a
  release-only project would compile a binary without it. The CLI always
  uses the dev profile, so this does not affect the schema-extract path,
  but it does mean release binaries never carry the dump machinery.
- **Version skew between the CLI and the user's `toasty`.** The dump
  envelope carries a format version; on mismatch the CLI reports it and
  asks the user to align the two, instead of failing on a parse error.
- **Builder-level schema options.** Options set on `Db::builder()` at
  runtime — currently `table_name_prefix` — are not visible to the
  constructor, which runs before any user code. Projects using them keep
  working at runtime, but the extracted schema does not include them.
- **Env var leaking to user processes.** The env var is set only on the
  child the CLI spawns, never exported in the user's shell. Users who
  manually `TOASTY_DUMP_SCHEMA=sqlite cargo run` get the dump-and-exit
  behavior too, which is the intended way to test the path.
- **Sandboxed or hardened-runtime macOS bins.** Constructors run normally
  in `cargo build` output. We do not support extracting from an externally
  signed and notarized release binary.

## Driver integration

Nothing for driver authors. The schema-extract path is entirely above the
`Driver` trait. `generate_migration` moves off the `Driver` trait entirely:
migration SQL is a function of the diff and the target `Capability`, so it
is generated by `toasty_sql::generate_migration` and drivers stay focused
on runtime database access. A driver that previously customized migration
SQL now expresses the difference through its `Capability`.

## Alternatives considered

**Synthesized dumper crate ([#824]).** Generates `target/toasty-dump/` with
a `Cargo.toml` that path-depends on the user's lib and a 6-line
`dumper.rs`. Works, but lib-only, and the manifest must mirror the user's
feature selection. The linktime approach uses the user's existing target,
no manifest mirroring, and handles bin-only.

**Dump the `app::Schema` and lower in the CLI.** Would make the dump
flavor-independent, but requires serde support across the whole
`app::Schema` graph — including `stmt::Value` and `stmt::Path` — and turns
an unstable, registration-order-dependent structure into a wire format
between separately versioned binaries. The `db::Schema` already has a
stable, versioned serialization (it is the snapshot format), and lowering
in the user's process keeps the user's `toasty-core` authoritative.

**Static extraction via `object` / `goblin`.** Read schema fragments out
of the linked binary without executing it. Requires every part of
`app::Schema` to be const-constructible, which it is not (`String`,
`Vec`, recursive trait dispatch through `BelongsTo<T>` and `Embed`).
Large refactor that loses the cross-type resolution that motivated
moving away from the original proc-macro side-effect design.

**Opt-in `dump_if_requested()` call site.** The user adds one line to
their `main.rs` and the CLI runs the user's binary with an env var. The
linktime ctor is the same idea with the call site removed; it also
generalizes to lib-only crates, which the call-site approach does not.

**Hand-rolled `rustc` invocation with `--extern` flags.** Build the user's
crate with Cargo to get rmeta/rlib paths, then invoke `rustc` directly
on a free-floating `dumper.rs`. Brittle: feature unification, proc-macro
host paths, and per-package `[profile]` settings have to be replicated by
hand.

**Inject an `examples/` target into the user's source tree.** Cargo
auto-discovers `examples/*.rs`. Mutates the user's source tree even
transactionally; rejected for the same reason in [#762].

## Out of scope

- **Watch mode.** Auto-regenerate migrations on save. Separate feature.
- **Cross-compilation.** The ctor approach assumes the dumper artifact
  runs on the host. Schema extraction for cross-compiled targets is not
  supported.
- **Schema export format.** The dump envelope is IPC between the CLI and
  the constructor, not a public schema format. A stable on-disk schema
  format is a separate concern.

[#762]: https://github.com/tokio-rs/toasty/issues/762
[#824]: https://github.com/tokio-rs/toasty/pull/824
[`linktime`]: https://docs.rs/linktime
