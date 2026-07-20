# DynamoDB driver error-handling convention

## Summary

This design establishes and enforces a consistent `Err`-vs-panic convention for the DynamoDB driver: a backend that cannot satisfy a legitimate request returns `Err`; a planner- or capability-gated impossibility panics. No user-facing API changes. The observable change is that several inputs that previously crashed the process now return an error the caller can match on and handle.

## Motivation

Toasty has no documented `Err`-vs-panic convention. `AGENTS.md`, the architecture docs, the DDB notes, and `toasty_core::Error` (18 purpose-built variants) are all silent on when an operation should return an error versus panic. The DDB driver reflects that gap: it mixes `Err`, `panic!`, `todo!`, and `assert!` with no consistent rule.

The correct rule is already implied by the driver's own best cases. When a backend genuinely cannot satisfy a legitimate request, the existing good sites return `Err(unsupported_feature)` — raw SQL, transactions, composite unique indices, and `.ilike()` all follow this pattern. When an operation is impossible because the planner or a capability gate already excluded it, the existing good sites use `unreachable!` with a message naming the gate — the collection-mutation arm in `update_by_key.rs` is the model.

Several user-reachable unsupported operations violate that rule by panicking instead:

* `.like()` against DynamoDB (`lib.rs`, in `ddb_expression`)
* `!=` on a primary key
* arithmetic in a condition expression
* `>1` unique index
* multiple keys combined with a unique index

These abort the process on input a caller could reasonably supply. The inconsistency surfaces directly in the user guide (`docs/guide/src/dynamodb.md`), which documents `.ilike()` as returning an `unsupported_feature` error and `.like()` as panicking — side by side, presented as facts to warn about rather than as a rule.

## The convention

|Situation|Use|Rationale|
|-|-|-|
|Reachable from user input or schema shape, and the backend genuinely can't do it|`Err` — `unsupported_feature`, or `invalid_statement` for a malformed request|Callers deserve an error they can handle; crashing the process is wrong|
|"Cannot happen — the planner/capability gate already excluded it"|`unreachable!` / `panic!`, with a message naming the gate|Reaching this path is always a bug; the message identifies the broken invariant|
|Not built yet|`todo!` as an internal marker is acceptable — but any limitation a user can encounter today must surface as `unsupported_feature`, not `todo!`/`panic!`|Standing limitations are returned errors, not crashes|

`unsupported_feature` is a variant of `toasty_core::Error`: a message-carrying error meaning "the database does not support a requested feature." It is the established idiom for this category — raw SQL, transactions, composite unique indices, and `.ilike()` all return it, and `AGENTS.md` names it as the convention for `.ilike()` (gated by `Capability::native_ilike`).

This document states the rule for the whole codebase, but this design converts only the DynamoDB sites. A repo-wide audit is out of scope.

## Behavior

**Convert to `Err`** (user-reachable, currently panic):

|Site|Today|Becomes|
|-|-|-|
|`.like()` against DynamoDB|`panic!`|`unsupported_feature`|
|`!=` on a primary key|`todo!`|`unsupported_feature`|
|arithmetic in a condition expression|`todo!`|`unsupported_feature`|
|`>1` unique index|`panic!` / `todo!`|`unsupported_feature`|
|multiple keys + a unique index|`assert!` / `panic!`|`unsupported_feature`|
|delete multi-key transaction-cancelled path|`todo!`|`condition_failed` / `driver_operation_failed`|

`.like()` is documented in the user guide as a panic; that guide section
(`docs/guide/src/dynamodb.md`) is updated to describe the returned error
instead.

**Stays a panic** (correct under the rule):

* the collection-mutation `unreachable!` in `update_by_key.rs` — capability-gated; this is the model message to copy.
* `generate_migration`'s `unimplemented!` — returns `Migration`, not `Result`, so it cannot return `Err`.

## Implementation

Every panic in the Behavior table lives inside `ddb_expression`, which currently returns `String`. Converting those sites requires changing the signature to `Result<String>` and threading `?` through the recursive calls and all five call sites: `query_pk.rs`, `scan.rs`, `find_pk_by_index.rs`, `update_by_key.rs`, and `delete_by_key.rs`. This is the bulk of the implementation effort and touches every caller, but it is a self-contained change with no dependencies on the open questions above.

## Edge cases

* **`value.rs` / `type.rs` panics are reachable but on non-`Result` signatures.** Converting them is a larger surface change (the conversion traits don't return `Result`). Tracked as follow-ups; not converted here.

## Alternatives considered

* **`invalid_statement` instead of `unsupported_feature` for the query-time cases.** `unsupported_feature`'s docstring frames it as a
  schema-construction/validation-time error ("a mismatch between application requirements and database capabilities"). Most of these conversions fail at query-execution time, not schema build. The variant's meaning still fits — the backend can't do the requested thing — and it is the established idiom, so the design uses it. The doc-review question is whether to broaden the variant's docstring to acknowledge query-time uses or split the query-time cases to `invalid_statement`. See open questions.
* **Leave the panics as documented behavior.** The guide already documents `.like()` panicking, so one option is to treat that as the contract. This is rejected: a panic is not a contract a library should hand its callers for ordinary unsupported input, and the side-by-side inconsistency with `.ilike()` is exactly the kind of surprise the convention exists to remove.

## Open questions

* **`unsupported_feature` vs `invalid_statement` for query-time failures — blocks acceptance.** The variant's docstring describes schema-time errors; most of these conversions are query-time. Decide whether to broaden the docstring to acknowledge query-time uses or route the query-time subset to `invalid_statement`. This is the primary question for doc review to settle.
* **Insert version guard on batch inserts — blocks implementation of the `insert.rs` change.** Multi-row insert uses `batch_write_item`, which cannot carry the `attribute_not_exists` version-column guard the single-row `put_item` path applies; for a versioned model the guard is silently dropped on batches. The `insert.rs` `todo!` → `Err` conversion touches this file. Decide whether to restore a transact path for the versioned-batch case or document the gap as intentional before that change lands.

## Out of scope

* **`value.rs` / `type.rs` panics** — reachable but on non-`Result`signatures; tracked as follow-ups.
* **Repo-wide `Err`-vs-panic audit** — the rule is written generally, but
  only the DDB sites are converted in this work.
* **New `toasty_core::Error` variants** — the existing set covers every
  conversion here.
