# Application Option semantics

Application optionality is distinct from database nullability. Option
presence follows Rust's rules; present values use database comparison rules.
The [design](../design/option-semantics.md) defines the contract motivated by
[#188](https://github.com/tokio-rs/toasty/issues/188).

Implementation proceeds in separate PRs for representation, query-result
presence, equality, membership, and adjacent relation and ordering behavior.
Each PR documents its supported behavior and includes regression coverage.

Completion requires:

- Option presence rules for equality and membership, including implicit
  `Some`, with boolean results in filters, projections, negation, and
  conditional writes. Present values keep their database equality.
- Independent field, row, relation, and loading presence; explicit rejection
  of stored types whose presence cannot be preserved.
- Consistent optional ordering and pagination, with native database
  operators retaining their documented contracts.
- Shared tests using Rust as the oracle for presence and the target database
  as the oracle for present values. Cover collation, NaN, empty inputs,
  nested presence, and agreement between filters and projections.

Database filters currently emit native comparisons. Constant folding and
client-side evaluation use Rust value comparisons; in-memory relation
matching also uses local hash and sort comparisons. These paths need review
where their equality can differ from the database's. An optimization or
choice of execution location must not change a query's result.

Cursor predicates must agree with the database's order and equality for
present values, including collation ties and PostgreSQL NaN. The existing
primary-key tie-breakers remain necessary when the requested order is not
unique.
