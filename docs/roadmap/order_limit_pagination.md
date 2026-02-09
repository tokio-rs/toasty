# Query Ordering, Limits & Pagination

> **User Guide:** See [guide/pagination.md](../guide/pagination.md) for complete usage examples and API documentation.

## Overview

Toasty provides cursor-based pagination using keyset pagination, which offers consistent performance and works well across both SQL and NoSQL databases. The implementation converts pagination cursors into WHERE clauses rather than using OFFSET, avoiding the performance issues of traditional offset-based pagination.

## Current Implementation

### Ordering (ORDER BY)

Single column ordering with ascending/descending direction:

```rust
let users = User::all()
    .order_by(User::FIELDS.created_at().desc())
    .collect(&db)
    .await?;
```

Multi-column ordering requires manual construction:

```rust
use toasty::stmt::OrderBy;

let order = OrderBy::from([
    Post::FIELDS.status().asc(),
    Post::FIELDS.created_at().desc(),
]);

let posts = Post::all()
    .order_by(order)
    .collect(&db)
    .await?;
```

### Cursor-based Pagination

Paginated queries return a `Page<T>` struct:

```rust
let page: Page<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;

// Access items
for post in &page.items {
    println!("{}", post.title);
}

// Navigate forward
if let Some(next_page) = page.next(&db).await? {
    process_posts(&next_page.items);
}

// Navigate backward
if let Some(prev_page) = page.prev(&db).await? {
    process_posts(&prev_page.items);
}
```

The `Page<T>` struct provides:
- `items: Vec<T>` - the results
- `next_cursor: Option<stmt::Expr>` - cursor for next page
- `prev_cursor: Option<stmt::Expr>` - cursor for previous page
- `.next(&db)` - fetch next page
- `.prev(&db)` - fetch previous page
- `.has_next()` / `.has_prev()` - check if more pages exist

Manual cursor usage:

```rust
let page = Post::all()
    .order_by(Post::FIELDS.id().desc())
    .paginate(10)
    .after(cursor)  // or .before(cursor)
    .collect(&db)
    .await?;
```

### Limit

The `Limit` struct exists and is used internally by pagination, but no direct `.limit()` method is exposed for non-paginated queries.

### First Convenience Method

Get the first matching record:

```rust
let first_user: Option<User> = User::all()
    .order_by(User::FIELDS.created_at().asc())
    .first(&db)
    .await?;
```

## Potential Future Work

### Multi-column Ordering Convenience

Add `.then_by()` method for chaining multiple order clauses:

```rust
let users = User::all()
    .order_by(User::FIELDS.status().asc())
    .then_by(User::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;
```

**Implementation:**
- File: `toasty-codegen/src/expand/query.rs`
- Add `.then_by()` method to query builder
- Complexity: Medium

### Direct Limit Method

Expose `.limit()` for non-paginated queries:

```rust
let recent_posts: Vec<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .limit(5)
    .collect(&db)
    .await?;
```

**Implementation:**
- File: `toasty-codegen/src/expand/query.rs`
- Generate `.limit()` method
- Complexity: Low

### Last Convenience Method

Get the last matching record:

```rust
let last_user: Option<User> = User::all()
    .order_by(User::FIELDS.created_at().desc())
    .last(&db)
    .await?;
```

**Implementation:**
- File: `toasty-codegen/src/expand/query.rs`
- Generate `.last()` method
- Complexity: Low

## Testing

### Existing Tests

Located in `crates/toasty-driver-integration-suite/src/tests/`:

- **Basic ordering** - `one_model_sort_limit.rs::sort_asc`
  - Tests ascending and descending ordering
  - Verifies correct sort order in results

- **Page-based pagination** - `one_model_sort_limit.rs::paginate`
  - Tests `.paginate()` with forward navigation
  - Tests `.after()` for cursor-based navigation
  - Tests `page.next()` and `page.prev()` methods

- **First convenience method** - `one_model_crud.rs` and others
  - Tests `.first()` returns first matching record
  - Tests `.first()` returns None when no matches

### Additional Test Coverage

Tests that could be added:

- **Multi-column ordering**
  - Verify correct ordering with multiple columns
  - Test tie-breaking behavior

- **Direct `.limit()` method** (when implemented)
  - Non-paginated queries with limit
  - Verify correct number of results

- **`.last()` convenience method** (when implemented)
  - Returns last matching record
  - Returns None when no matches

- **Edge cases**
  - Empty results with pagination
  - Single page results (no next/prev cursors)
  - Pagination beyond last page
  - Large page sizes
  - Page size of 1

## Database-Specific Considerations

### SQL Databases

- **MySQL:** Uses `LIMIT n` for pagination (keyset approach via WHERE)
- **PostgreSQL:** Uses `LIMIT n` for pagination (keyset approach via WHERE)
- **SQLite:** Uses `LIMIT n` for pagination (keyset approach via WHERE)

All SQL databases use keyset pagination (WHERE clauses with cursors) rather than OFFSET for consistent performance.

### NoSQL Databases

- **DynamoDB:**
  - Limited ordering support (only on sort keys)
  - Pagination via LastEvaluatedKey
  - Cursor-based approach maps well to DynamoDB's native pagination
  - Needs validation and testing

## How Keyset Pagination Works

Instead of using `OFFSET`, Toasty converts cursors to `WHERE` clauses:

```sql
-- Traditional OFFSET (slow for large offsets)
SELECT * FROM posts ORDER BY created_at DESC LIMIT 10 OFFSET 10000;

-- Toasty's cursor approach (always fast)
SELECT * FROM posts
WHERE (created_at, id) < ('2024-01-15 10:30:00', 12345)
ORDER BY created_at DESC, id DESC
LIMIT 10;
```

This provides:
- **Consistent Performance:** O(log n) regardless of page number
- **Stable Results:** New records don't shift pagination boundaries
- **Database Agnostic:** Works efficiently on NoSQL databases
- **Real-time Friendly:** Handles concurrent insertions gracefully

## Notes

- The core pagination infrastructure is production-ready for applications
- Cursors (`stmt::Expr`) can be serialized at the application level if needed for web APIs
- Pagination requires an explicit ORDER BY clause to ensure consistent results
- Multi-column ordering works today via manual `OrderBy` construction
- The `.then_by()` convenience method would improve ergonomics but isn't essential
