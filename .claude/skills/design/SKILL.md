---
name: design
description: Always use this skill before authoring or editing a Toasty design document under docs/dev/design/
---

# Authoring Toasty Design Documents

Load this skill before writing or editing a design document in
[`docs/dev/design/`](../../../docs/dev/design/).

## Format and layout

**Always read [`docs/dev/design/_template.md`](../../../docs/dev/design/_template.md)
before writing.** It is the authoritative source for section order,
section purposes, and the framing the doc should adopt. Copy the
template to `docs/dev/design/<feature-name>.md` and fill it in. Keep the
section order; if a section genuinely does not apply, delete it and
explain why in one line rather than leaving it empty.

## Writing style

Follow the conventions from the [`prose`](../prose/SKILL.md) skill: be
fact-focused, direct, and concrete. No buzzwords, no fluff, no dramatic
terms.

## Be succinct

Readers already know Toasty and Rust. Lead with the problem and the
proposal so a maintainer can grasp the important bits quickly. Cut
restated background, obvious explanations, and throat-clearing.
Length is not a virtue; clarity is.

## Rust examples

Rust code blocks in design docs are illustrative. They **do not need to
compile** and are **not** run through rustdoc or any other test.

Do **not** add `#`-prefixed hidden boilerplate — no `# use …` imports, no
`# async fn __example(…) { … }` wrappers, no `# fn main`, nothing hidden
to satisfy a compiler. Show only the lines that matter to the reader, even
if the snippet would not compile on its own.

This overrides the doctest-preamble instruction in `_template.md`'s
`User-facing API` section: that preamble is for the user guide, where
examples *are* tested. Design docs are not.

## Framing

A design doc is **guide-level**, not implementation-level. Write it for
the two audiences the template names:

- Toasty users — Rust developers writing models and queries.
- Driver implementors — anyone implementing the `Driver` trait.

Describe what those audiences will see, call, and have to do. Omit
internal module layouts and implementation choices that have no
observable effect on either audience. The `User-facing API` section
should read like a chapter of the user guide — prose with worked
examples, not an API catalog. The examples are still illustrative only
(see `Rust examples` above) — do not add doctest boilerplate to them.

## Workflow

Non-trivial features follow the path in
[`CONTRIBUTING.md`](../../../CONTRIBUTING.md): open a feature-proposal
issue first, then land a roadmap entry in
[`docs/dev/roadmap/`](../../../docs/dev/roadmap/) and the design doc
**in the same PR**. The implementation lands as a follow-up PR once the
design is accepted.
