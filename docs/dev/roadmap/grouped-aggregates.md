# Grouped counts and extrema

Give model list queries a typed grouped-result surface for COUNT(*), MIN,
and MAX, with GROUP BY and HAVING. The
[design](../design/grouped-aggregates.md) narrows the first implementation
under [#421](https://github.com/tokio-rs/toasty/issues/421). Scalar `.count()`
and field/tuple projections already exist; the issue's missing-count claim
is stale. SUM/AVG and the rest of the issue remain follow-up work.

SQL drivers execute one native grouped query. DynamoDB returns
`unsupported_feature` before data access, with no implicit scan fallback.
The contract includes PostgreSQL, SQLite, MySQL/MariaDB, and Turso's
SQLite modes, and preserves existing model-query behavior.

Completion requires:

- A distinct grouped builder returning typed scalar/tuple projections,
  count/min/max expressions, and HAVING over keys and aggregates.
- Validation of grouping membership and clause order, nullable keys and
  extrema, checked result conversion, and native collation behavior.
- Native SQL serialization, strict MySQL grouping, existing scalar-count
  regression coverage, and DynamoDB rejection with zero data requests.
- An explicit account of public AST source compatibility for out-of-tree
  users before adding grouped clauses or new aggregate nodes.

Land the design first; implementation follows in a separate PR referencing
it. This entry does not mark all aggregate work in #421 complete.
