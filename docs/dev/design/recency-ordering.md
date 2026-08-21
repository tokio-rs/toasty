# Recency Ordering (`newest_by` / `oldest_by`)

## Summary

Two new query methods, `.newest_by(field)` and `.oldest_by(field)`,
give a typed shortcut for the common "most/least recent row" query.
They are sugar over `.order_by(field.desc()/.asc()).first()`, restricted
at compile time to fields whose type is meaningfully orderable by
recency via a new marker trait, `stmt::Recency`. The restriction is
enforced through the type system, not through a runtime check, so
calling `.newest_by(User::fields().name())` is a compile error rather
than a query that silently does the wrong thing.

## Motivation

`.order_by(...)` is fully general but says nothing about intent, and
nothing stops a caller from ordering by a column that isn't a
meaningful proxy for time:

```rust
User::all()
    .order_by(User::fields().name().desc())
    .first()
```

This compiles and runs, returning whichever user sorts last
alphabetically almost certainly not what the caller meant when they
were trying to find the most recently created user. The bug is easy to
write (`.desc()` vs `.asc()` is also a frequent typo source) and
invisible at the call site; nothing about `order_by` signals "this is
supposed to mean newest."

"Give me the newest X" is also one of the most common queries written
against any model with a timestamp, and today it requires remembering
which direction `Desc` corresponds to "newest," then chaining `.first()`
separately.

## User-facing API

### Finding the most recent row

Call `.newest_by(...)` with a field handle for a recency-orderable
column, then terminate the query as usual:

```rust
let newest: Option<User> = User::all()
    .newest_by(User::fields().created_at())
    .first()
    .exec(&mut db)
    .await?;
```

`.newest_by(...)` composes with `.filter(...)` and `.limit(...)`
exactly like `.order_by(...)` does:

```rust
let recent_active: Vec<User> = User::all()
    .filter(User::fields().active().eq(true))
    .newest_by(User::fields().created_at())
    .limit(10)
    .exec(&mut db)
    .await?;
```

### Finding the oldest row

`.oldest_by(...)` is the symmetric counterpart, ordering ascending
instead of descending:

```rust
let first_signup: Option<User> = User::all()
    .oldest_by(User::fields().created_at())
    .first()
    .exec(&mut db)
    .await?;
```

### What fields are accepted

Only fields whose type implements `stmt::Recency` are accepted. Today
that means timestamp-shaped types — under the `jiff` feature,
`jiff::Timestamp`, `jiff::Zoned`, `jiff::civil::DateTime`, and
`jiff::civil::Date`; a matching `chrono` feature gate covers
`chrono::DateTime<Utc>` and `chrono::NaiveDateTime`. Ordering by an
arbitrary `String`, `i64`, or `Uuid` field does not compile:

```rust
// compile error: `String` does not implement `Recency`
User::all().newest_by(User::fields().name())
```

If you need to order by something `Recency` does not cover today (an
auto-increment id, a UUIDv7), use `.order_by(...)` directly — see
"Out of scope" for why those are not in current implemntation.

### Before and after

Today, finding the most or least recent row means spelling out
direction explicitly with `.order_by(...)` and remembering that
`Desc` means newest-first, `Asc` means oldest-first:

```rust
// Before: most recent row
User::all()
    .order_by(User::fields().created_at().desc())
    .first()

// Before: least recent (oldest) row
User::all()
    .order_by(User::fields().created_at().asc())
    .first()
```

Both lines compile identically whether `created_at` is actually a
recency-appropriate column or not — `.order_by(...)` accepts any
orderable field, so `.order_by(User::fields().name().desc())` is just
as valid and just as easy to write by mistake when "most recent" was
the intent.

With `.newest_by(...)` / `.oldest_by(...)`, the direction is implied by
the method name and the field is restricted to recency-orderable types
at compile time:

```rust
// After: most recent row
User::all()
    .newest_by(User::fields().created_at())
    .first()

// After: least recent (oldest) row
User::all()
    .oldest_by(User::fields().created_at())
    .first()
```

Existing `.order_by(...)` calls are untouched and remain the right
choice for ordering by non-recency fields, multi-column sorts, or any
case `Recency` doesn't cover (see "Out of scope"). `.newest_by(...)`
and `.oldest_by(...)` are additive, not a replacement for `.order_by(...)`.

## Behavior

**Happy path.** `.newest_by(field)` sets the query's order-by clause to
a single descending sort on `field`; `.oldest_by(field)` sets an
ascending sort. Both fully replace any existing order-by on the query,
identically to how a second `.order_by(...)` call would.

**Error cases.** There is no runtime error path specific to this
feature. Rejection of non-recency fields happens entirely at compile
time via the `Recency` bound; there is nothing left to validate at
query build or execution time.

**Interactions.**
- *`.order_by(...)`.* Calling `.order_by(...)` after `.newest_by(...)`
  (or vice versa) overwrites the prior call; only the last one applies.
  This matches existing `.order_by(...)` semantics and is not new
  behavior introduced by this feature.
- *`.limit(...)`.* Composes normally; `.newest_by(...).limit(n)` returns
  the n most recent rows.
- *Pagination.* No interaction beyond what `.order_by(...)` already has
  with cursor-based pagination; `.newest_by(...)` produces an ordinary
  single-column sort that pagination already knows how to encode into a
  cursor.
- *Relations / includes.* No interaction; `.newest_by(...)` only
  affects row ordering of the query it's called on.
