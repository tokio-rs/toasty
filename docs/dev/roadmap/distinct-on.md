# Latest row per group

Select one complete source row per group using PostgreSQL DISTINCT ON,
with ordering that determines the winner. This addresses the
latest/first-per-group use cases in [#425](https://github.com/tokio-rs/toasty/issues/425).
The [design](../design/distinct-on.md) defines the proposed public API
and driver contract. Implementation follows acceptance of that design.

The checked upstream tip already serializes plain SELECT DISTINCT and
uses it in relation loading. This item adds DISTINCT ON to model queries;
it does not replace that behavior or complete all of #425.

Completion requires:

- Scalar and tuple source-model keys, explicit group-prefix ordering,
  complete model hydration, and typed scalar projections of winning rows.
- Filters before selection and limits/offsets after selection; stable ties
  when users provide a unique ordering; includes scoped to winning models.
- A native DISTINCT ON capability enabled for PostgreSQL and explicit
  pre-dispatch errors on SQLite, MySQL/MariaDB, and DynamoDB.
- Rejection of cursor pagination, aggregation/counting, row locks, and
  mutation conversion until their compositions have accepted contracts.
- Coverage of ties, null keys, composite groups, collation, projections,
  includes, unsupported backends, and query-level ordering boundaries.

Coordinate with the parallel aggregate proposal for
[#421](https://github.com/tokio-rs/toasty/issues/421): selecting a row is
different from computing a grouped maximum. Neither proposal requires
the other to merge first; same-level composition is initially rejected.
