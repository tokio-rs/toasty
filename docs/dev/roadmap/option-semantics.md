# Application Option semantics

Application optionality is distinct from database nullability.
The [design](../design/option-semantics.md) defines the contract motivated by
[#188](https://github.com/tokio-rs/toasty/issues/188).

Implementation proceeds in separate PRs for representation, query-result
presence, equality, membership, and adjacent relation and ordering behavior.
Each PR documents its supported behavior and includes regression coverage.

Completion requires:

- Rust equality and membership, including implicit `Some`, with boolean
  results in filters, projections, negation, and conditional writes.
- Independent field, row, relation, and loading presence; explicit rejection
  of stored types whose presence cannot be preserved.
- Consistent optional ordering and pagination, with native database
  operators retaining their documented contracts.
- Shared tests using Rust as the oracle across supported SQL and NoSQL
  execution paths, including empty inputs, nested presence, and payload
  equality affected by collation or NaN.
