# Data-Carrying Enum Implementation Design

Builds on unit enum support (#355). See `docs/design/enums-and-embedded-structs.md`
for the user-facing design.

## DynamoDB

Equivalent encoding to be determined when implementing the DynamoDB driver phase.

## Remaining Work

- **Partial updates**: within-variant partial update builder.

- **DynamoDB**: equivalent encoding in the DynamoDB driver.

## Open Questions

- **`SparseRecord` / `reload`**: within-variant partial updates are supported, so
  `SparseRecord` and `reload` are needed for enum variant fields. Determine how
  `reload` should handle a `SparseRecord` scoped to a specific variant's fields —
  the in-memory model must update only the changed fields without disturbing the
  discriminant or other variant columns.

- **Shared columns**: variants sharing a column via `#[column("name")]` is in the
  user-facing design. Schema parsing should record shared columns in Phase 1; full
  query support is a follow-on.
