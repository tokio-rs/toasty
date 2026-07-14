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

## Be succinct

Maintainers reading the issue already know Toasty and Rust. Keep the
prose high-signal: lead with the bug or proposal, then the reproducer
or API sketch, then the alternatives. Skip restated background and
throat-clearing. A maintainer should grasp the problem in seconds, not
paragraphs.

## Pick the right template

Issue templates live in
[`.github/ISSUE_TEMPLATE/`](../../../.github/ISSUE_TEMPLATE):

- [`bug_report.yml`](../../../.github/ISSUE_TEMPLATE/bug_report.yml) —
  incorrect or unexpected behavior. Asks what you did, what you expected,
  and what happened instead; the affected driver(s); the Toasty version;
  and a reproducer.
- [`feature_proposal.yml`](../../../.github/ISSUE_TEMPLATE/feature_proposal.yml) —
  new features, public-API changes, or anything that affects driver
  implementations. Requires the problem, proposed solution,
  alternatives considered, and a scope estimate.

Read the template before writing. Fill in every field it asks for;
don't drop sections or leave placeholders. If a field does not apply,
say so explicitly rather than leaving it blank.

## Bug reports

Report what you observed, not why you think it happened. A bug report
has five parts:

- **What you did** — the query or operation you ran.
- **What you expected** — the result you expected.
- **What actually happened** — the error, wrong result, or panic. Quote
  the exact message, generated SQL, or backtrace.
- **Additional context** — affected driver, Toasty version, related
  issues.
- **A minimal reproducer** — a failing test in
  `crates/toasty-driver-integration-suite/src/tests/` is ideal; a small
  standalone snippet works too. This is the single most useful thing in
  the report.

Leave out root-cause analysis. Diagnosing the bug is the maintainer's
job, and a guess at the cause sends triage down the wrong path. Describe
the behavior; the reproducer shows the rest.

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
