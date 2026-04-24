---
name: issue
description: Always use this skill before opening an issue in the Toasty repository
---

# Opening Issues

Load this skill before filing any issue in this project.

## Writing style

An issue is project documentation. Follow the conventions from the
[`prose`](../prose/SKILL.md) skill: be fact-focused, direct,
and concrete. No buzzwords, no fluff, no dramatic terms. State what
happened or what is being proposed, not how important it is.

## Pick the right template

Issue templates live in
[`.github/ISSUE_TEMPLATE/`](../../../.github/ISSUE_TEMPLATE):

- [`bug_report.yml`](../../../.github/ISSUE_TEMPLATE/bug_report.yml) —
  incorrect or unexpected behavior. Requires a reproducer, the affected
  driver(s), and the Toasty version or commit SHA.
- [`feature_proposal.yml`](../../../.github/ISSUE_TEMPLATE/feature_proposal.yml) —
  new features, public-API changes, or anything that affects driver
  implementations. Requires the problem, proposed solution,
  alternatives considered, and a scope estimate.

Read the template before writing. Fill in every field it asks for;
don't drop sections or leave placeholders. If a field does not apply,
say so explicitly rather than leaving it blank.

## Bug reports

A small, self-contained reproducer is the single most useful thing in a
bug report. A failing test in
`crates/toasty-driver-integration-suite/src/tests/` is ideal; a
standalone snippet works too. Include the exact error, generated SQL,
or backtrace when relevant.

## Feature proposals

Describe the problem before the solution. Name who is affected —
Toasty users, driver implementors, or both. Sketch the user-facing API
concretely; vague proposals are hard to discuss. List the alternatives
you considered and why you discarded them.

Non-trivial features follow the path in
[`CONTRIBUTING.md`](../../../CONTRIBUTING.md): discuss in an issue,
land a roadmap entry and design doc, then land the implementation.

## Labels

Do not apply labels when creating the issue. The templates set the
initial `C-*` label; maintainers triage and add the rest. See
[`docs/dev/labels.md`](../../../docs/dev/labels.md).
