# Array-Driven As-Of Joins

> **Working draft.** This document explores a single concrete query end to
> end. The pieces it identifies (table-valued array inputs, lateral joins,
> custom row projections) will eventually split into separate design docs
> and merge into the `query!` macro doc. Treat the syntax here as
> illustrative; specific spellings are open questions.

## Summary

Toasty has no way to express the dashboard / time-series query pattern
"for each of N sample points, fetch the most recent related row that was
current at that point". Doing it today requires either fetching every
candidate row to Rust and joining in memory, or dropping to raw SQL.
This doc walks through one such query, identifies the three building
blocks needed to support it (table-valued inputs from Rust collections,
lateral joins, and custom row projections), and proposes a `query!`
spelling for each.

## Motivation

A common UI pattern is a chart with N sample points along the X axis and
M series along the Y axis: portfolio value over time per asset, server
load per host per minute, inventory level per SKU per day. The shape of
the underlying query is always the same:

1. Generate a grid of `(sample_point, series)` rows on the application
   side.
2. For each row in that grid, look up the most recent stored snapshot
   whose timestamp is `<=` the sample point.
3. Project a chart-friendly row that combines the grid coordinates with
   the snapshot columns, using `0` (or another default) when no snapshot
   exists for that grid cell.

Toasty cannot express any of step 1, 2, or 3 today:

- **Step 1** has no representation. The application can build the grid
  in Rust, but there is no way to feed it into a query as a relation.
  Binding one parameter per cell does not scale: PostgreSQL's wire
  protocol caps parameter count at 65 535, and a query whose SQL text
  changes with input length pollutes `pg_stat_statements` with one
  entry per length.
- **Step 2** is a lateral join with `ORDER BY ... LIMIT 1`. Toasty has
  no AST node for `LATERAL` and no builder API that produces one
  ([#419](https://github.com/tokio-rs/toasty/issues/419)).
- **Step 3** is a custom row shape — not a model instance, not a single
  scalar. The builder always returns whole models. The aggregation
  proposal ([#421](https://github.com/tokio-rs/toasty/issues/421))
  hints at this need but does not address it directly.

## Reference query

The rest of this doc works through one query from end to end: an
inventory dashboard that charts on-hand stock at a given warehouse for
several SKUs at a sequence of sample timestamps.

### Model

```rust
use chrono::{DateTime, Utc};

#[derive(Debug, toasty::Model)]
struct InventoryLevel {
    #[key]
    id: i64,

    warehouse_id: String,
    sku: String,
    recorded_at: DateTime<Utc>,

    on_hand: i64,
    reserved: i64,

    received_total: i64,
    shipped_total: i64,
}
```

`InventoryLevel` rows are append-only snapshots written each time a
warehouse event changes the totals.

### Target SQL

The query takes three inputs from Rust: `warehouse_id: &str`, `skus:
&[String]`, and `sample_times: &[DateTime<Utc>]`. It returns one row
per `(sample_time, sku)` pair containing the snapshot that was current
at that time, or zeros if none.

```sql
SELECT
    s.recorded_at,
    s.sku,
    b.recorded_at                                          AS matched_at,
    COALESCE(b.on_hand,  0)                                AS on_hand,
    COALESCE(b.reserved, 0)                                AS reserved,
    COALESCE(b.received_total - b.shipped_total, 0)        AS net_flow
FROM   UNNEST($1::timestamptz[]) AS s_t(recorded_at)
CROSS  JOIN UNNEST($2::text[])   AS s_k(sku)
LEFT   JOIN LATERAL (
    SELECT recorded_at, on_hand, reserved, received_total, shipped_total
    FROM   inventory_levels h
    WHERE  h.warehouse_id = $3
      AND  h.sku          = s_k.sku
      AND  h.recorded_at <= s_t.recorded_at
    ORDER BY h.recorded_at DESC
    LIMIT 1
) b ON TRUE;
```

Three things to notice. The grid arrives as two parameters
(`$1` `timestamptz[]`, `$2` `text[]`); array length is data, not part of
the SQL text. `LATERAL` lets the inner subquery reference `s_t` and
`s_k` columns. The outer `SELECT` is a custom shape — neither a model
nor a single scalar.

### The same query in `query!`

```rust
# use chrono::{DateTime, Utc};
# use toasty::query;
# async fn __example(
#     mut db: toasty::Db,
#     warehouse_id: &str,
#     skus: &[String],
#     sample_times: &[DateTime<Utc>],
# ) -> toasty::Result<()> {
let chart = query!(
    SELECT
        s.recorded_at,
        s.sku,
        b.recorded_at                                AS matched_at,
        COALESCE(b.on_hand,  0)                      AS on_hand,
        COALESCE(b.reserved, 0)                      AS reserved,
        COALESCE(b.received_total - b.shipped_total, 0) AS net_flow
    FROM
        INPUTS(recorded_at = #sample_times, sku = #skus) AS s
        LEFT JOIN LATERAL (
            InventoryLevel
            FILTER  .warehouse_id == #warehouse_id
                AND .sku          == s.sku
                AND .recorded_at  <= s.recorded_at
            ORDER BY .recorded_at DESC
            LIMIT 1
        ) AS b
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

`chart` is a `Vec<_>` of an anonymous (or generated) row type with
fields `recorded_at`, `sku`, `matched_at`, `on_hand`, `reserved`,
`net_flow`. See [Return shape](#return-shape) for what that type
actually is.

## Building blocks

The query needs three new pieces. They are independent of each other
and useful on their own.

### 1. Table-valued inputs from Rust collections

`INPUTS(col_a = #vec_a, col_b = #vec_b, ...)` produces a relation whose
rows are the Cartesian product of the input slices, with one named
column per slice. Each `#vec` resolves to any `IntoIterator<Item = T>`
where `T` has a known Toasty SQL type.

The single-column form `UNNEST(#vec) AS name(col)` is the primitive;
`INPUTS(...)` is sugar for several `UNNEST` cross-joined together.
Single-column is enough for "for each id in this set, find ...":

```rust
# use toasty::query;
# async fn __example(mut db: toasty::Db, ids: &[i64]) -> toasty::Result<()> {
query!(
    SELECT .id, .name
    FROM   User
    FILTER .id IN UNNEST(#ids)
)
# .exec(&mut db).await?;
# Ok(())
# }
```

The driver lowering is what makes this worth building (see
[Driver lowering](#driver-lowering)). The defining property is that
the SQL text **does not depend on slice length**: a 3-element slice and
a 30 000-element slice produce the same SQL, with the slice itself
passed as a single bound array parameter. This avoids the PostgreSQL
65 535-parameter wire-protocol cap and keeps `pg_stat_statements` from
accumulating one entry per slice length.

### 2. Lateral joins

`LEFT JOIN LATERAL ( <subquery> ) AS alias` runs `<subquery>` once per
row of the preceding `FROM` clause. The subquery body is itself a
`query!` source, so it gets the full filter / order-by / limit
vocabulary. Inside the subquery, paths starting with `.` resolve
against the joined model; paths qualified with an outer alias
(`s.sku`) resolve against the outer row.

`JOIN LATERAL` (inner) and `LEFT JOIN LATERAL` (outer) are both
supported. Other join kinds (`RIGHT`, `FULL`) are out of scope; their
semantics with `LATERAL` are constrained or unsupported in most
backends.

### 3. Custom row projection (`SELECT`)

`SELECT <expr> [AS <name>], ...` declares the output row. Without a
`SELECT` clause, `query!` keeps its current behavior — return the model
type. With a `SELECT` clause, the return type is determined by the
projection list. Expressions in the projection list can include:

- Column references, qualified by alias when ambiguous (`b.on_hand`,
  `s.recorded_at`).
- Arithmetic on numeric columns (`b.received_total - b.shipped_total`).
- `COALESCE(expr, default)` and other built-in functions.
- Toasty external references (`#expr`) for constants and bound
  parameters.

This is the same expression language already needed for filters; the
projection list extends it to the output side.

## Behavior

### Return shape

For the worked example, the return type is a `Vec` of rows with six
fields. There are three viable spellings; this doc proposes the first
and lists the others under [Open questions](#open-questions).

**Generated struct, named after the binding.** The macro generates a
struct whose name is derived from the `let` binding (here, `Chart`)
with one field per projection alias. This requires the macro to see
the binding, which limits where `query!` can appear (no inline
expressions) but produces the most ergonomic call site.

```rust
let chart: Vec<Chart> = query!(SELECT ...).exec(&mut db).await?;
// where
//   struct Chart { recorded_at: DateTime<Utc>, sku: String, ... }
```

### Empty inputs

If any `UNNEST` input is empty, the Cartesian product is empty, and the
query returns an empty `Vec` without contacting the database. The
planner short-circuits.

### No matching snapshot

`LEFT JOIN LATERAL` returns one row per outer row regardless of whether
the lateral subquery matched. Unmatched rows have `NULL` for every
column from the lateral side, which `COALESCE` in the projection turns
into the supplied default. With `JOIN LATERAL` (inner), unmatched outer
rows are dropped.

### Type checking

Each `#vec` reference must resolve to an `IntoIterator<Item = T>` whose
`T` has a Toasty SQL type. Mismatches between the projection
expressions and the declared return shape are caught at compile time by
the macro expansion.

## Driver lowering

### `UNNEST` / `INPUTS`

The lowered form depends on driver capability:

| Backend | Lowering | Bind shape |
|---|---|---|
| PostgreSQL | `UNNEST($n::T[])` | one array parameter per `#vec` |
| MySQL 8+ | `JSON_TABLE($n, '$[*]' COLUMNS (val ...))` | one JSON parameter per `#vec` |
| SQLite | `json_each($n)` | one JSON-text parameter per `#vec` |
| DynamoDB | client-side Cartesian product, then per-row `Query` | no array binding |

For SQL backends, the contract is one bound parameter per `#vec`,
regardless of length. SQL drivers expose this via a new `Capability`
flag (`array_input: bool`); drivers that opt out fall back to the
parameter-per-element form, with a warning in the planner when the
slice exceeds a configurable threshold.

### `LATERAL`

PostgreSQL emits `LEFT JOIN LATERAL ( ... ) ON TRUE` directly. SQLite
3.45+ supports the same syntax. MySQL 8.0.14+ supports it. For
backends that do not, the engine can emulate by issuing one
`Operation::Query` per outer row at the exec layer, similar to the
existing nested-merge mechanism. A `Capability::lateral_join` flag
gates this.

DynamoDB has no `LATERAL`; the engine emulates by issuing one query
per outer row.

### Custom projection

For SQL drivers, the projection list serializes inline as a `SELECT`
list of column expressions. For DynamoDB, the projection runs in the
exec layer after rows return — the underlying request always returns
full attribute sets; column-level projection happens in Rust.

## Alternatives considered

**Bind one parameter per element.** A `WHERE x IN ($1, $2, ..., $N)`
form is what Toasty would naturally generate today if extended
naively. Rejected for the two reasons in [Motivation](#motivation):
the PostgreSQL wire-protocol parameter cap, and the proliferation of
distinct SQL texts in `pg_stat_statements`.

**Pre-sample in Rust.** The grid-and-snapshot join could happen in
Rust after fetching every candidate snapshot. This works for small
data but defeats the point — for a chart with 64 sample points and 5
SKUs over a snapshot history of millions of rows, the per-row index
seek (made possible by `LATERAL`) is dramatically faster than dragging
the history to the application.

**Raw SQL escape hatch only.** Tell users to drop to raw SQL for these
queries. Rejected: the patterns here (array input, lateral, custom
projection) are common enough to warrant first-class support, and an
escape hatch loses Toasty's compile-time type checking and driver
portability.

**Type-inferred source `#vec AS alias(col)`.** Drop the `UNNEST`
keyword; let the macro detect that `#vec` is a `Vec<T>` and treat it as
a table-valued source automatically. More concise but harder to read
and dispatches on a trait the user has to know about. Keeping the
explicit `UNNEST(...)` matches SQL convention and is easy to teach.

## Open questions

- **Return shape**. Generated struct (named from the binding), tuple of
  named columns, or a user-supplied struct that the macro projects
  into? **Blocks acceptance** — the spelling of `SELECT` depends on
  this choice.
- **`INPUTS` Cartesian default vs. zip**. `INPUTS(...)` cross-joins by
  default. Some callers want zipped pairs (i-th time with i-th sku).
  Add `ZIP(...)` later or require explicit `UNNEST` + join? Deferrable.
- **Naming**. `UNNEST` matches Postgres but is unfamiliar to SQLite
  users. Alternatives: `ROWS`, `VALUES`, `EACH`. Deferrable.
- **Capability advertising**. Should `array_input` and `lateral_join`
  be separate `Capability` flags, or a single "advanced query" flag?
  Deferrable.
- **Threshold for parameter-per-element fallback**. When a driver
  lacks `array_input`, at what slice length does Toasty warn or
  refuse? Configurable per `Db`? Deferrable.

## Out of scope

- **Aggregations** (`GROUP BY`, `HAVING`, `SUM`, `AVG`, `MIN`, `MAX`).
  Covered separately by [#421](https://github.com/tokio-rs/toasty/issues/421).
  The expression and projection machinery designed here is a
  prerequisite, but the aggregation surface is its own design.
- **Window functions**. Adjacent to lateral joins but a distinct
  feature with a much larger surface.
- **Full join taxonomy** (`RIGHT JOIN`, `FULL JOIN`, plain
  non-lateral `JOIN ... ON`). The `query!` macro handles ordinary
  relations through `INCLUDE` and `EXISTS`; explicit join syntax can
  follow once these patterns demand it.
- **Subqueries in expressions** (scalar subqueries in `SELECT` /
  `WHERE`). Useful but orthogonal — the as-of pattern motivates
  lateral, not scalar subqueries.
