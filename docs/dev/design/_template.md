<!--
Design document template.

Copy this file to `docs/dev/design/<feature-name>.md` and fill it in.
Delete sections that genuinely do not apply (and explain why in one line),
but keep the section order.

A design doc is **guide-level**: write it for the people who will use the
change, not for someone reading the implementation. The two audiences are:

  - Toasty users — Rust developers writing models and queries.
  - Driver implementors — anyone implementing the `Driver` trait.

Describe what those audiences will see and have to do. Avoid documenting
internal modules or implementation choices that have no observable effect.
-->

# {Feature name}

## Summary

One paragraph: what changes for users, and why this is worth doing.

## Motivation

The problem this solves. Concrete user scenarios that are awkward,
impossible, or surprising today. Quote real user reports or issues if you
have them.

## User-facing API

Write this section as a chapter of the Toasty user guide — prose that can
be adapted into [`docs/guide/src/`](../../guide/src/) once the feature
ships. Introduce the concept, then show idiomatic usage with code
examples. Tell the reader what to call, when to reach for it, and how it
fits with features they already know.

Match the existing guide's style: narrative prose broken up by `##`
subsections, Rust code blocks, and direct instructional voice ("Call
`.foo()` on a query…"). Favor worked examples over bullet-list API
catalogs.

Code blocks here are illustrative only — they do not need to compile and
are not tested. Do **not** add doctest boilerplate (`# use …`,
`# async fn __example(…) { … }`, etc.); show only the lines that matter.
The doctest preamble belongs in the user guide, not in design docs.

When this changes existing API, include a short "Before and after"
showing how user code migrates.

See [`docs/guide/src/sorting-limits-and-pagination.md`](../../guide/src/sorting-limits-and-pagination.md)
and [`docs/guide/src/batch-operations.md`](../../guide/src/batch-operations.md)
for the target tone.

## Behavior

How the API behaves at runtime. Include:

- Happy path — what the user gets back.
- Error cases — what the user sees when things go wrong, and what type of
  error it is.
- Defaults and implicit behavior the user does not control.
- Interactions with other features (relations, embedded types, includes,
  pagination, transactions — whichever apply).

## Edge cases

Cases that affect user code or driver code and are easy to get wrong:

- Boundary values, empty inputs, null handling.
- Concurrency or ordering assumptions.
- Behavior across Toasty's two worlds — SQL vs. NoSQL, single-statement
  vs. batched, etc.

## Driver integration

What driver implementors need to do. When the answer is "nothing," state
that explicitly so future driver authors do not re-derive it.

- Required `Driver` capabilities or new capability flags.
- New `Operation` variants the driver must handle.
- SQL serialization contract for SQL drivers (per-dialect notes if they
  differ).
- Encoding contract for non-SQL drivers (e.g. DynamoDB attribute shapes).
- Backward compatibility for out-of-tree drivers.

## Alternatives considered

Other designs you weighed and why this one won. A short paragraph each.
Save the next reader from re-running the same analysis.

## Open questions

Decisions still open. Mark each one as blocking acceptance, blocking
implementation, or deferrable.

## Out of scope

Things a reader might expect to find here but this design does not
cover, each with one line on why. Keeps review focused.
