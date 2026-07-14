---
name: sync-docs
description: Bring the Toasty user guide and rustdoc back in sync with recent code changes by walking the git log for user-observable behavior and updating affected docs
---

# Syncing Toasty Documentation

Load this skill when asked to update, sync, refresh, audit, or catch up
Toasty's documentation. Toasty iterates quickly, so both the user guide
under [`docs/guide/src/`](../../../docs/guide/src/) and the rustdoc in
the public crates routinely drift behind the code. This skill describes
the sweep used to bring them back in line.

## The two documentation surfaces

Both are public and both can drift:

- **User guide** — `docs/guide/src/*.md`, rendered via mdBook. Tutorial
  and reference for Rust developers writing models and queries.
- **Rustdoc** — module- and item-level docs on public APIs across
  workspace crates. The two crates with the largest exposed surface are
  [`crates/toasty/src/lib.rs`](../../../crates/toasty/src/lib.rs)
  (end-user ORM API) and
  [`crates/toasty-core/src/driver.rs`](../../../crates/toasty-core/src/driver.rs)
  (driver-implementor API). Driver implementors are public users — the
  `Driver` and `Connection` traits, `Operation` enum, and `Capability`
  are part of the contract that must stay documented.

## Workflow

### 1. Pick a scan range

Find a sensible starting point by looking at when each surface was last
touched:

```
git log --oneline -- docs/guide/src/
git log --oneline -- crates/toasty/src/lib.rs crates/toasty-core/src/
```

The earlier of those two HEADs is a reasonable lower bound for the
sweep. If the user gave a date or commit, use that instead. Then list
the candidate commits:

```
git log --oneline <since>..HEAD
```

### 2. Triage each commit for user-observable impact

For every commit in the range, decide whether it changes something a
**user or driver implementor** can see. Skim the subject; if uncertain,
look at the diff. Categories that usually need a doc touch:

- **Public API additions or changes** — new model attributes, new
  methods on `Db` / generated builders / `BelongsTo` / `HasMany` /
  `HasOne`, new items in `stmt::*` or `schema::*`, changes to function
  signatures users call.
- **Newly supported field types or storage shapes** — e.g. adding
  `Vec<scalar>`. Both the guide chapter (defining models, field
  options) and the rustdoc on the relevant types need to mention it.
- **Driver-facing changes** — anything touching `toasty-core/src/driver.rs`,
  the `Operation` enum, `Capability`, or the contract a `Connection`
  must uphold. These need rustdoc updates so third-party driver authors
  see the new rules.
- **Behavior changes observable from outside** — connection pool
  semantics, transaction handling, when queries fail vs. succeed,
  cascading deletes, etc. If a user could write a test that distinguishes
  before from after, it counts.
- **Default or configuration changes** — anything that changes what
  happens when the user does not set an option.

Categories that usually do **not** require a doc update:

- Pure internal refactors with no observable change (e.g. engine IR
  restructuring, simplifier rule consolidation).
- Test-only or CI-only commits.
- Renovate / dependency bumps that do not change semver-visible API.

Build a short list of "commits → which doc(s)" before editing anything.

### 3. Find existing claims that the change makes stale

A change can break docs without adding new surface. Search the guide
and rustdoc for terms tied to the change before writing anything new:

```
rg -t md '<term>' docs/guide/src/
rg '<term>' crates/*/src/
```

The recent `docs: correct DynamoDB uniqueness enforcement claim` commit
is the canonical example — the code's behavior had changed and a
sentence in the guide was now wrong. Update or delete stale claims as
part of the sweep.

### 4. Write the updates

Always invoke the [`prose`](../prose/SKILL.md) skill before authoring
or editing any markdown or rustdoc prose. The same conventions apply
to rustdoc comments: present tense, active voice, fact-focused, no
buzzwords, document only current behavior.

When updating the guide, match the chapter's existing register and
example style. When adding rustdoc on items that already have doc
comments, extend in place rather than appending an afterthought
paragraph.

Do **not** narrate the change ("recently added", "now supports", "as of
0.x.y"). Describe what the feature is, not when it arrived.

Do **not** reference design docs from code or guide prose — design docs
under `docs/dev/design/` are deleted once the feature lands.

### 5. Verify the docs build

```
cargo doc --no-deps --workspace
```

For the guide, if `mdbook` is installed:

```
mdbook build docs/guide
```

If `mdbook` is not available, at minimum run `cargo build` so any
`#![doc = include_str!(...)]` references in `lib.rs` still resolve, and
spot-check the edited markdown files for valid links and code fences.

Rustdoc examples that aren't marked `ignore` are compiled — broken
examples fail the doc build, so verify after edits.

## Scope discipline

This skill is a maintenance sweep, not a rewrite. Touch only what the
recent commits actually invalidate or leave undocumented. If the sweep
surfaces a larger gap (a chapter that was never written, a whole
subsystem with no rustdoc), flag it to the user rather than expanding
scope mid-sweep.
