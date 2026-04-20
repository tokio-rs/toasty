---
name: pr
description: Always use this skill before opening a pull request in the Toasty repository
---

# Opening Pull Requests

Load this skill before opening a pull request in this project.

## Base the title and body on the full branch diff

Draft the title and body from the **full diff of the current branch against
`main`**, not from the latest commit or the last few commits. Use
`git diff main...HEAD` and `git log main..HEAD` to see everything the PR
will introduce. A PR often bundles several commits (fixups, review
response, refactors) — the title and body describe the net change that
lands on `main`, not the most recent work-in-progress.

## Title

The PR title follows the same Conventional Commits format as commit
messages, because it becomes the squash-merge commit message. Read
[`docs/dev/COMMITS.md`](../../../docs/dev/COMMITS.md) when drafting the
title and make sure it stands on its own.

## Body

Fill in the PR body using the template at
[`.github/pull_request_template.md`](../../../.github/pull_request_template.md).
Keep the section headings and the checklist; replace the HTML comment
placeholders with real content. Delete checklist items that do not apply
rather than leaving them unchecked with no explanation.

## Labels

**Do not apply labels when creating the PR.** Maintainers triage and
label issues and PRs separately — see
[`docs/dev/labels.md`](../../../docs/dev/labels.md). Passing `--label`
to `gh pr create` or setting labels in any other way bypasses that
process.
