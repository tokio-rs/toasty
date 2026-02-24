---
name: write-docs
description: Author or edit documentation for the Toasty project, following project writing conventions
---

# Writing Toasty Documentation

Load this skill when writing or editing any documentation in the `docs/` directory or other markdown files in this project.

## Writing Style

- **Be fact-focused**: State what things are and what they do
- **Avoid buzzwords**: No "leverage", "synergy", "paradigm", etc.
- **Avoid fluff**: Every sentence should convey information
- **Avoid business jargon**: No "stakeholders", "deliverables", "action items"
- **Avoid weasel words**: No "very", "really", "quite", "somewhat"
- **Avoid dramatic terms**: No "critical", "crucial", "vital", "essential" unless something will actually break
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

## Document Structure

- Start with what the thing is
- Explain why it exists (what problem it solves)
- Explain what it does
- Show how to use it (if applicable)
- Provide examples
