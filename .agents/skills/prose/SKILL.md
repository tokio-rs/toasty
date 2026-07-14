---
name: prose
description: Author or edit any prose for the Toasty project — documentation, design docs, READMEs, PR descriptions, issue bodies, commit message bodies, or other human-readable text — following project writing conventions
---

# Writing Toasty Prose

Load this skill whenever writing or editing prose for this project: documentation in `docs/`, READMEs, design docs, PR descriptions, issue bodies, commit message bodies, or any other human-readable markdown.

## Writing Style

- **Be fact-focused**: State what things are and what they do
- **Avoid buzzwords**: No "leverage", "synergy", "paradigm", etc.
- **Avoid fluff**: Every sentence should convey information
- **Avoid business jargon**: No "stakeholders", "deliverables", "action items"
- **Avoid weasel words**: No "very", "really", "quite", "somewhat"
- **Avoid dramatic terms**: No "critical", "crucial", "vital", "essential" unless something will actually break
- **Avoid figurative metaphors**: Pick the literal word for the thing, not the analogy. "Features light up on PostgreSQL" → "Toasty enables features on PostgreSQL". "Query shape" → "query pattern" or "query form". Other recurring offenders: "under the hood" (just describe what's there), "out of the box" (just say "by default"), "first-class" (say what's actually supported), "magic" (say what the code does). If you can't replace the metaphor with a literal noun or verb without losing meaning, you probably don't know what you mean yet.
- **Be direct**: Say what you mean without hedging
- **Use concrete examples**: Show, don't tell
- **Use active voice**: "The engine executes queries" not "Queries are executed by the engine"
- **Use present tense**: Describe how the system works now, not how it was designed or how it will work
- **Document current behavior only**: Omit historical decisions, deprecated approaches, and planned future work

### Examples

**Bad**: "This component is critical for ensuring optimal query performance."

**Good**: "This component optimizes queries by combining multiple database round-trips into one."

**Bad**: "The simplification phase leverages various transformations to enhance query efficiency."

**Good**: "The simplification phase rewrites association traversals into explicit subqueries."

**Bad**: "Native arrays light up on PostgreSQL for `Vec<scalar>` fields."

**Good**: "On PostgreSQL, `Vec<scalar>` fields use native array columns (`text[]`, `int8[]`, …)."

**Bad**: "## Query shapes that work"

**Good**: "## Supported queries"

## Document Structure

- Start with what the thing is
- Explain why it exists (what problem it solves)
- Explain what it does
- Show how to use it (if applicable)
- Provide examples
