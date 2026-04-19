# Contributing to Toasty

This document describes how to propose and land changes. The process is
lightweight, and the right path depends on the size of the change. Use
your judgment. When in doubt, open a `C-discussion` issue and ask.

## Two paths

### Small or obvious changes — just send a PR

Open a pull request directly for bug fixes, docs improvements, internal
cleanups, ergonomic tweaks, new tests, or any change that does not affect
the public API. No prior discussion needed.

The trade-off: a PR opened without prior discussion may be closed without
detailed review if it does not align with the project's direction. If you
are about to spend significant time on a change, open an issue first.

### Larger changes — propose, get on the roadmap, then implement

For new features, public-API changes, anything that affects driver
implementations, or anything that will take more than a small amount of
work, the recommended path is:

1. **Propose.** Open an issue (`C-feature` or `C-discussion`) describing
   the problem and a sketch of the approach.
2. **Get on the roadmap.** Land a PR that:
   - Adds an entry under [`docs/dev/roadmap/`](docs/dev/roadmap/), and
   - Adds a guide-level design document under
     [`docs/dev/design/`](docs/dev/design/). Start from
     [`_template.md`](docs/dev/design/_template.md).

   Both go in the same PR. Reviewers debate and refine the design on that
   PR. It contains no implementation.
3. **Implement.** Once the design PR merges, open a follow-up PR with the
   implementation. The merged design doc is the contract; reviewers
   should not need to re-litigate decisions made there.

This is a recommendation, not a hard rule. Maintainers may waive steps
for small, obvious changes, and may request the full process for changes
that look small but turn out not to be.

## Using AI assistants

Using an AI assistant to write code, issue descriptions, or PR
descriptions is welcome. The human author is responsible for
understanding the submitted code and defending it in review. A PR whose
author cannot discuss the change gets closed.

## Triage

Maintainers close issues and PRs without detailed review when a change
does not align with the project's direction, duplicates existing work,
or is not worth the time to review. Closures are routine and carry no
judgment of the contributor. If you have context that changes the
picture, reopen, follow up in the thread, or ping a maintainer in the
`#toasty` channel on the [tokio-rs Discord][discord] to continue the
discussion.

## What "guide-level" means

A design document under `docs/dev/design/` is **guide-level**: write it
as a guide for the people who will use the change, not as an internal
implementation note. Those people are:

- **Toasty users** — Rust developers writing models and queries.
- **Driver implementors** — anyone implementing the `Driver` trait,
  whether in-tree or out-of-tree.

The design doc describes what these audiences see and do:

- The public API surface introduced or changed (types, functions,
  attributes, macros).
- User-facing behavior, including ergonomics and error cases.
- Edge cases that affect user code or driver code.
- Required driver capabilities and what driver authors must implement.
- SQL/NoSQL serialization contracts where they are observable.

Do not re-document internal modules, code organization, or implementation
choices with no effect on either audience. Those notes belong in code
comments or an architecture doc once the work has shipped.

## Conventions

### Commit and PR titles

This repository follows the [Conventional Commits][cc] specification.
Every commit message and PR title uses the form:

```
type: description
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`,
`build`, `ci`, `chore`, `revert`. Scopes are optional. The subject begins
with a lowercase letter.

[cc]: https://www.conventionalcommits.org/

### Before you push

```bash
cargo fmt
cargo clippy
cargo test
```

`cargo test` runs the SQLite-backed integration suite and needs no
external services. When you touch driver code, also run the suite against
the affected driver — see [`CLAUDE.md`](CLAUDE.md) for the cargo
invocations.

### Labels

Maintainers triage issues and apply labels from the scheme in
[`docs/dev/labels.md`](docs/dev/labels.md). Contributors do not need to
apply labels.

## Where to ask

- **Discord:** [tokio-rs Discord][discord] — the `#toasty` channel is the
  fastest way to ask a question or float an idea before opening an issue.
- **Issues:** open a `C-discussion` issue for anything that needs more
  than a chat exchange.

[discord]: https://discord.gg/tokio
