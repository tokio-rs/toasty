---
name: commit
description: Always use this skill before authoring a commit message in the Toasty repository
---

# Authoring Commit Messages

Load this skill before writing any git commit message in this project.

**Always read [`docs/dev/COMMITS.md`](../../../docs/dev/COMMITS.md) before
authoring a commit message.** It is the authoritative source for the
format, allowed types, scope conventions, subject/body/footer rules, and
breaking-change notation.

## Be succinct

Maintainers reading the log already know Toasty and Rust. Keep the
subject and body high-signal: state what changed and why. Skip
restated context, obvious explanations, and throat-clearing. A
maintainer should grasp the important bits in seconds.
