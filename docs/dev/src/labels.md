# GitHub Labels

This document defines the label taxonomy for issues and pull requests in the
`tokio-rs/toasty` repository.

Labels use prefixes borrowed from `rust-lang/rust`:

- `C-` category (what kind of issue)
- `A-` area (which part of the codebase)
- `P-` priority
- `I-` impact
- `S-` status

Most issues carry one `C-` label, zero or more `A-` labels, and a `P-` label
once triaged. `I-` and `S-` are applied when they apply.

## `C-` Category

What kind of work the issue represents.

| Label | Use for |
|---|---|
| `C-bug` | Something is broken or behaves incorrectly |
| `C-feature` | New feature request or missing use case |
| `C-enhancement` | Improvement to an existing feature |
| `C-refactor` | Internal code change with no user-visible effect |
| `C-docs` | User docs, dev docs, rustdoc |
| `C-chore` | CI, release tooling, scripts, lint fixes |
| `C-tracking` | Umbrella issue tracking related work |
| `C-discussion` | Open-ended design discussion or RFC |

Every issue should carry exactly one `C-` label.

## `A-` Area

Which part of the codebase the issue touches.

| Label | Maps to |
|---|---|
| `A-engine` | `toasty/src/engine/` — simplify, lower, plan, exec |
| `A-macros` | `toasty-macros` and the `create!` / `models!` macros |
| `A-schema` | `toasty-core/src/schema/` — app, db, mapping |
| `A-sql` | `toasty-sql` — AST to SQL serialization |
| `A-driver` | Driver trait, capabilities, connection lifecycle, and per-backend behavior (SQLite, PostgreSQL, MySQL, DynamoDB) |
| `A-migration` | `toasty-cli` and migration generation or application |
| `A-tests` | `toasty-driver-integration-suite` and workspace tests |
| `A-docs` | `docs/` mdbook content |
| `A-ci` | `.github/workflows`, release-plz, `scripts/` |

An issue may carry zero, one, or several `A-` labels. Leave `A-` off when
nothing fits — do not force-fit. Add new `A-` labels when a cluster of issues
shares a theme that none of the existing labels cover.

## `P-` Priority

Urgency and blast radius.

| Label | Use for |
|---|---|
| `P-critical` | Data loss, data corruption, severe correctness, or security |
| `P-high` | Blocks a user with no workaround |
| `P-medium` | Default for actionable issues |
| `P-low` | Nice-to-have |

## `I-` Impact

Nature of a bug when it warrants specific attention beyond priority.

| Label | Use for |
|---|---|
| `I-unsound` | Soundness hole (undefined behavior, violated safety invariants) |
| `I-security` | Security vulnerability — apply alongside `P-critical` |

## `S-` Status

Workflow state. Apply when it helps route attention; clear when it no longer
applies.

| Label | Use for |
|---|---|
| `S-needs-design` | Requires a design decision before implementation |
| `S-needs-repro` | Bug report lacks a reproducer |
| `S-needs-info` | Waiting on more information from the reporter |
| `S-blocked` | Blocked on another issue, PR, or upstream change |
| `S-waiting-on-author` | PR awaiting author changes |
| `S-waiting-on-review` | PR awaiting maintainer review |

## Community

GitHub built-in labels for outside contributors. Keep the default names so
GitHub's UI highlights them.

| Label | Use for |
|---|---|
| `good first issue` | Beginner-friendly, well-scoped, mentorable |
| `help wanted` | Maintainers welcome outside contributions |

## Examples

- A DynamoDB query planner bug with a clear reproducer: `C-bug`, `A-engine`,
  `A-driver`, `P-high`.
- A request to support native Postgres enums: `C-feature`, `A-driver`,
  `A-schema`.
- An umbrella issue tracking simplification rules: `C-tracking`, `A-engine`.
- A typo fix in `docs/dev/src/architecture.md`: `C-docs`, `A-docs`.
- A release-plz config change: `C-chore`, `A-ci`.
- A cross-cutting API naming discussion: `C-discussion` — no `A-` needed.
