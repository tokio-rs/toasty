# Implementing Option semantics

Application options and database nulls have separate representations and
operation semantics. The [design](../design/option-semantics.md) defines the
contract; the [user guide](../../guide/src/optional-values.md) describes the API.

## 1. Preserve application types and values

`Type::Option`, `Value::Option`, and `Expr::OptionSome` retain application
presence. `Load::app_ty()` exposes application types independently of the
storage representation returned by `Load::ty()`. Application field metadata
also exposes optionality separately from database column nullability.

Typed values construct `Some` explicitly. `IntoComparison` lifts compatible
required expressions and paths when an optional comparison expects them,
without making ordinary projection type inference ambiguous. Lowering encodes
options according to the field mapping before dispatching database operations.

Stored nested options fail compilation, including nesting through aliases and
transparent pointer wrappers. Query result cardinality remains independent:
`.first()` on an optional projection preserves `Some(None)`.

## 2. Lower equality and boolean composition

Typed comparisons carry an application marker until lowering. Application
`eq` compares presence first and present payloads second; `ne` negates the
complete predicate. Application predicates produce booleans in filters,
projections, and conditional writes. Embedded values compare their application
members, including optional members. Variant guards belong inside negation.

String comparisons use binary operands on MySQL and MariaDB to preserve case
and trailing spaces. PostgreSQL floating-point comparisons exclude NaN
payloads. Drivers that cannot store NaN without losing its value reject it.

Native database expressions retain null propagation. Internal client filters
treat an unknown database predicate as a non-match, which preserves relation
joins without equating absent foreign keys.

## 3. Lower membership

List membership compares candidates using application equality. Empty lists,
duplicates, absent candidates, tuples, and negation follow Rust `contains`.
Native `IN` and array binding remain available when their behavior agrees.

Optional subquery membership uses null-safe `EXISTS`. A derived table preserves
the candidate query's limit, offset, and ordering before correlation. Key-value
plans compare values in the bound result list using the same presence rules.
An optional subject accepts a query returning required candidates by lifting
those candidates into `Some`.

Collection element support remains governed by `Scalar`: stored collections
of optional elements are rejected by the existing trait bounds. Native array
operators retain their documented contracts for supported element types.

## 4. Complete adjacent API behavior

Generated relation field handles retain their target type, building on
[PR #1249](https://github.com/tokio-rs/toasty/pull/1249). Optional relations expose
presence methods. Comparisons with a model resolve the declared reference
fields, including relations that reference a non-primary unique key.

Optional string predicates return false for absence. Present values retain
native `LIKE`/`ILIKE` behavior and capability checks. Ordered comparisons place
`None` before `Some`; sorting and cursor predicates agree on absence placement
in both page directions.

Create defaults, omitted update assignments, and explicit `None` remain
separate operations. Deferred state, row presence, field presence, and relation
lookup also remain separate.

## 5. Publish the contract and migration path

The guide and rustdoc describe application presence with `Option`. Database
null terminology is reserved for storage mappings and native operators.
`Expr::some()` constructs an option. The arbitrary `Expr::cast()` retag is
deprecated; `cast_unchecked()` names the low-level compatibility operation.

Callers should review result-set changes for inequality, negation, membership,
variant predicates, and optional ordering. Existing single-option field storage
does not require a schema migration.

## Verification

The shared `option_semantics` suite uses Rust comparisons as its oracle. It
covers scalar truth tables, boolean fields and projections, literal and
subquery membership, optional embeds, relation presence and non-primary
references, string predicates, sorting, pagination, writes, and row cardinality.
No contract tests are ignored. Driver capability gates restrict unsupported
query forms, without permitting different Option semantics.

`tests/tests/option_expr.rs` checks application construction and evaluation
against the separate database-null evaluator. UI tests cover resolved nested
option rejection and optional relation target types. Existing relation,
embedding, binding, and pagination suites provide regression coverage beyond
the new contract tests.

The 32 shared Option contract tests run alongside the existing driver suites.
Verification passes on all five local backends:

| Backend | Passed | Failed | Ignored |
|---|---:|---:|---:|
| SQLite | 873 | 0 | 0 |
| PostgreSQL | 904 | 0 | 0 |
| MySQL | 861 | 0 | 0 |
| MariaDB | 861 | 0 | 0 |
| DynamoDB | 683 | 0 | 0 |

Core, Toasty, and SQL tests and rustdoc pass with Tokio's `rt-multi-thread`
feature enabled for the runtime examples. The user guide's 184 executable
examples pass. Expression construction, generated relation APIs, compile-fail
diagnostics, and compile-pass examples also pass. Workspace Clippy reports no
warnings; formatting and whitespace checks pass.

## Related issues

| Issue | Implementation |
|---|---|
| [#188](https://github.com/tokio-rs/toasty/issues/188) | Application equality and membership preserve absence before database simplification. |
| [#797](https://github.com/tokio-rs/toasty/issues/797) | Application values and query cardinality preserve nested presence; unsupported stored nesting is rejected. |
| [#1246](https://github.com/tokio-rs/toasty/issues/1246) | Optional relation handles preserve their target and expose presence predicates. |
| [#1219](https://github.com/tokio-rs/toasty/issues/1219) | Comparing a relation with a model resolves its declared reference fields. |

Lossless stored nested options and opaque public database AST/cursor adapters
remain separate extensions; the supported behavior is rejection and explicit
low-level access, respectively.
