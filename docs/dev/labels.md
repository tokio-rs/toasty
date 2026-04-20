# GitHub Labels

Labels categorize issues and pull requests so maintainers and contributors can
filter by kind of work, affected area, priority, and workflow state.

The scheme uses prefixes:

- `C-` category — what kind of issue
- `A-` area — which part of the codebase
- `P-` priority
- `I-` impact
- `S-` status

Each issue has one `C-` label and, once triaged, one `P-` label. `A-`, `I-`,
and `S-` labels are applied when they fit.

## `C-` Category

What kind of work the issue represents.

| Label | Use for |
|---|---|
| `C-bug` | A defect: the code behaves incorrectly |
| `C-feature` | A new feature request or missing use case |
| `C-enhancement` | An improvement to an existing feature |
| `C-refactor` | An internal code change with no user-visible effect |
| `C-docs` | User docs, dev docs, rustdoc |
| `C-chore` | CI, release tooling, scripts, lint fixes |
| `C-tracking` | An umbrella issue that tracks related work |
| `C-discussion` | Open-ended design discussion or RFC |
| `C-sketch` | A proof-of-concept PR that demonstrates an idea and is not intended to be merged |

## `A-` Area

Which part of the codebase the issue touches.

| Label | Covers |
|---|---|
| `A-engine` | `toasty/src/engine/` — simplify, lower, plan, exec |
| `A-macros` | `toasty-macros` and the `create!` and `models!` macros |
| `A-schema` | `toasty-core/src/schema/` — app, db, mapping |
| `A-sql` | `toasty-sql` — AST to SQL serialization |
| `A-driver` | Driver trait, capabilities, connection lifecycle, and per-backend behavior (SQLite, PostgreSQL, MySQL, DynamoDB) |
| `A-migration` | `toasty-cli` and migration generation and application |
| `A-tests` | `toasty-driver-integration-suite` and workspace tests |
| `A-docs` | `docs/` mdbook content |
| `A-ci` | `.github/workflows`, release-plz, `scripts/` |

An issue carries zero, one, or several `A-` labels. Leave `A-` off when
nothing fits rather than force-fitting an existing label. Add a new `A-` label
when a group of issues shares a theme that none of the existing labels cover.

## `P-` Priority

How urgent the issue is.

| Label | Use for |
|---|---|
| `P-critical` | Data loss, data corruption, correctness, or security |
| `P-high` | Blocks a user with no workaround |
| `P-medium` | The default for actionable issues |
| `P-low` | Nice-to-have |

## `I-` Impact

The nature of a bug that needs attention beyond its priority.

| Label | Use for |
|---|---|
| `I-unsound` | A soundness hole: undefined behavior or a violated safety invariant |
| `I-security` | A security vulnerability — apply alongside `P-critical` |

## `S-` Status

Where the issue or PR sits in the workflow. Apply when it helps route
attention, and remove when it no longer applies.

| Label | Use for |
|---|---|
| `S-needs-design` | A design decision is required before implementation |
| `S-needs-repro` | The bug report lacks a reproducer |
| `S-needs-info` | Waiting on more information from the reporter |
| `S-blocked` | Blocked on another issue, PR, or upstream change |
| `S-waiting-on-author` | PR is waiting on author changes |
| `S-waiting-on-review` | PR is waiting on maintainer review |

## Community

GitHub built-in labels for contributions from outside the core team. Keep the
default names so GitHub's UI highlights them.

| Label | Use for |
|---|---|
| `good first issue` | A beginner-friendly, well-scoped issue |
| `help wanted` | Maintainers welcome a contributor to pick this up |

## Examples

- A DynamoDB query planner bug with a clear reproducer: `C-bug`, `A-engine`,
  `A-driver`, `P-high`.
- A request for native PostgreSQL enums: `C-feature`, `A-driver`, `A-schema`.
- An umbrella issue tracking simplification rules: `C-tracking`, `A-engine`.
- A typo fix in `docs/dev/architecture/README.md`: `C-docs`, `A-docs`.
- A release-plz config change: `C-chore`, `A-ci`.
- A cross-cutting API naming discussion: `C-discussion` — no `A-` needed.
