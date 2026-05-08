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

Today, using Toasty's migration tooling requires writing a per-project CLI
binary that links the user's models and dispatches to `toasty-cli` as a
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

`toasty migrate generate` is the only subcommand that needs the user's
schema. It compiles the user's package and reads the schema back out of the
build artifact. Other subcommands operate on saved migration files or talk
to a database directly.

### Workspaces

In a workspace, the CLI uses the workspace root package by default. Use
`-p <pkg>` to select a different member, mirroring `cargo`:

```
toasty -p api migrate generate --flavor postgresql
```

The flag is passed through to `cargo metadata` and the subsequent build, so
its semantics match Cargo's exactly. If the workspace has no root package
(a virtual manifest) and `-p` is not supplied, the CLI errors with the list
of workspace members.

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
   `cargo build --bin <name>`. When multiple bins exist, `--bin <name>`
   selects one explicitly; otherwise the CLI errors with the list of bin
   names.
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
#[cfg_attr(debug_assertions, ctor)]
fn __toasty_maybe_dump_schema() {
    if std::env::var_os("TOASTY_DUMP_SCHEMA").is_none() {
        return;
    }
    toasty::__dump_schema_to_stdout();
    std::process::exit(0);
}
```

This runs before `main` (for binaries) or during `dlopen` (for cdylibs).
When the env var is set, it walks the same `inventory` registrations
`#[derive(Model)]` already produces, builds an `app::Schema`, serializes
it as JSON to stdout, and exits with status 0. When the env var is not
set, it returns immediately.

The ctor is gated on `cfg(debug_assertions)` so release builds carry no
schema-dump machinery at all.

That ctors fire reliably from a `cargo rustc --crate-type cdylib` build
on Linux, macOS, and Windows is verified by
`tests/tests/cdylib_ctor_smoke.rs`, which CI runs against
`ubuntu-latest`, `macos-latest`, and `windows-latest`.

### Running the bin path

For a bin target, the CLI invokes the artifact directly:

```
TOASTY_DUMP_SCHEMA=1 ./target/debug/<bin-name>
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
toasty __load-cdylib /path/to/libuser_app.dylib
```

The subcommand body is roughly:

```rust
unsafe {
    libloading::Library::new(path)?;
}
unreachable!("ctor should have exited the process");
```

Setting `TOASTY_DUMP_SCHEMA=1` in the child's environment causes the ctor
to dump and exit during `Library::new`. The parent CLI captures the
child's stdout the same way it captures a bin's stdout. One binary is
shipped; the re-exec keeps the dump happening in a process the parent
controls.

The `__load-cdylib` subcommand is hidden from `--help` and not part of
the public surface — its only caller is `toasty-cli` itself.

### End-to-end flow for `migrate generate`

1. `cargo metadata` to identify the target package and its layout.
2. Build the chosen artifact (`--bin <name>` or
   `cargo rustc --crate-type cdylib`), parsing
   `--message-format=json-render-diagnostics` to find the artifact path.
3. Invoke the dumper: spawn the bin, or re-exec `toasty __load-cdylib`,
   with `TOASTY_DUMP_SCHEMA=1` in the environment.
4. Deserialize stdout as `app::Schema`.
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
  the CLI errors with "schema dumper produced no output, check that
  `toasty` is a direct dependency of `<pkg>`."
- **`toasty` not actually depended on.** The ctor is in `toasty`; without
  the dependency the env var has no effect. The CLI detects this in
  `cargo metadata` and errors before building.
- **Release builds.** The ctor is `cfg(debug_assertions)`-gated, so a
  release-only project would compile a binary without it. The CLI always
  uses the dev profile, so this does not affect the schema-extract path,
  but it does mean release binaries never carry the dump machinery.
- **Env var leaking to user processes.** The env var is set only on the
  child the CLI spawns, never exported in the user's shell. Users who
  manually `TOASTY_DUMP_SCHEMA=1 cargo run` get the dump-and-exit behavior
  too, which is the intended way to test the path.
- **Sandboxed or hardened-runtime macOS bins.** Constructors run normally
  in `cargo build` output. We do not support extracting from an externally
  signed and notarized release binary.

## Driver integration

Nothing for driver authors. The schema-extract path is entirely above the
`Driver` trait. SQL serialization for `migrate generate` already moves
from `Driver::generate_migration` to `toasty_sql::Flavor::generate_migration`
in [#824] and that change is preserved here — drivers stay focused on
runtime database access.

## Alternatives considered

**Synthesized dumper crate ([#824]).** Generates `target/toasty-dump/` with
a `Cargo.toml` that path-depends on the user's lib and a 6-line
`dumper.rs`. Works, but lib-only, and the manifest must mirror the user's
feature selection. The linktime approach uses the user's existing target,
no manifest mirroring, and handles bin-only.

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

## Open questions

- **`cfg(debug_assertions)` vs. always-on ctor.** A `getenv` per startup
  is cheap; gating on `debug_assertions` is cleaner. Keeping the gate
  means a release-only consumer cannot extract a schema from their built
  artifact, which is acceptable for a dev-time tool. Deferrable.
- **Subcommand surface for `--bin <name>`.** Likely just `toasty migrate
  generate --bin <name>`, mirroring `cargo`. Deferrable until the build
  selection logic lands.

## Out of scope

- **Watch mode.** Auto-regenerate migrations on save. Separate feature.
- **Cross-compilation.** The ctor approach assumes the dumper artifact
  runs on the host. Schema extraction for cross-compiled targets is not
  supported.
- **Schema export format.** This design extracts the same `app::Schema`
  the runtime uses, serialized as JSON. A stable on-disk schema format
  is a separate concern.

[#762]: https://github.com/tokio-rs/toasty/issues/762
[#824]: https://github.com/tokio-rs/toasty/pull/824
[`linktime`]: https://docs.rs/linktime
