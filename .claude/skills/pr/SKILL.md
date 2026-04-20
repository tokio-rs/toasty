---
name: pr
description: Always use this skill before opening a pull request in the Toasty repository
---

# Opening Pull Requests

Load this skill before opening a pull request in this project.

## Base the title and body on the PR diff, not the latest commit

A branch usually contains several commits — initial work, fixups,
review responses, rebases. The PR title and body describe the **net
change that will land on the base branch**, not the most recent commit.

Identify the PR's base branch first. It is usually `main`, but a PR
stacked on another feature branch has that feature branch as its base.
Then read the full diff against the base:

```
git diff <base>...HEAD
git log <base>..HEAD
```

Draft the title and body from what that diff actually contains.

## Title

Follows the same Conventional Commits format as a commit message — it
becomes the squash-merge commit. See
[`docs/dev/COMMITS.md`](../../../docs/dev/COMMITS.md).

## Body

Fill in the template at
[`.github/pull_request_template.md`](../../../.github/pull_request_template.md).
Keep the section headings and the checklist; replace the HTML comment
placeholders with real content. Delete checklist items that do not
apply rather than leaving them unchecked with no explanation.

## Labels

Do not apply labels when creating the PR. Maintainers triage and label
PRs separately — see [`docs/dev/labels.md`](../../../docs/dev/labels.md).
Passing `--label` to `gh pr create` bypasses that process.
