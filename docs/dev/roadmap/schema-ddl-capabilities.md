# Schema DDL capabilities

Let schema push and migrations honor a target's DDL transaction restrictions,
index creation syntax, and asynchronous readiness without weakening
application transactions. The [design](../design/schema-ddl-capabilities.md)
is proposed in [#1329](https://github.com/tokio-rs/toasty/issues/1329), with
Aurora DSQL as one example of a constrained PostgreSQL-compatible backend.

Implementation follows acceptance of the design. Completion requires:

- Generic schema capabilities and driver-owned rendering/completion hooks,
  with explicit opt-in profiles and a compatible legacy driver path.
- Agreement between schema push, offline generation, CLI application, and
  embedded migrations on target requirements and physical step boundaries.
- Legal DDL transaction groups, including isolated history-table DDL and
  bookkeeping DML where a backend forbids mixing them.
- Success only after confirmed index readiness; bounded waits and errors
  distinguishing confirmed failure, pending work, and unknown outcomes.
- Explicit non-atomic acknowledgement, durable incomplete-migration detection,
  and no replay of possibly committed DDL or already completed steps.
- Preserved PostgreSQL/SQLite atomic behavior, honest MySQL implicit-commit
  semantics, and no change to DynamoDB's native schema provisioning.
- Coverage of unique/non-unique async indexes, dependency order, unsupported
  plans before effects, physical statement counting, history insertion,
  conflicting DDL, timeouts, cancellation, and crash/connection-loss windows.

The query transaction work in [#1128](https://github.com/tokio-rs/toasty/issues/1128)
and [#1302](https://github.com/tokio-rs/toasty/pull/1302) remains separate.
