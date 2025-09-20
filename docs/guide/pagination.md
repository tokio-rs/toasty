# Pagination

Toasty provides efficient cursor-based pagination that works consistently across SQL and NoSQL databases. This approach avoids the performance issues of traditional offset-based pagination while providing a clean, web-friendly API.

## Basic Pagination

### Current Implementation

Basic pagination uses the `.paginate()` method to specify page size:

```rust
use toasty::Model;

// Get the first page of posts
let page = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;

// The page contains items and navigation cursors
println!("Found {} posts", page.items.len());

// Navigate to next page if available
if let Some(next_cursor) = page.next_cursor {
    let next_page = Post::all()
        .order_by(Post::FIELDS.created_at().desc())
        .paginate(10)
        .after(Some(next_cursor))
        .collect(&db)
        .await?;
}
```

### Page Structure

Paginated queries return a `Page<T>` struct containing the results and navigation cursors:

```rust
pub struct Page<T> {
    pub items: Vec<T>,                    // The page results
    pub next_cursor: Option<Cursor>,      // Navigate forward (None = last page)
    pub prev_cursor: Option<Cursor>,      // Navigate backward (None = first page)
}
```

## Navigation Patterns

### Forward Navigation

The most common pagination pattern - moving forward through results:

```rust
let mut current_page = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;

// Continue through pages
while let Some(next_cursor) = current_page.next_cursor {
    current_page = Post::all()
        .order_by(Post::FIELDS.created_at().desc())
        .paginate(10)
        .after(Some(next_cursor))
        .collect(&db)
        .await?;

    process_posts(&current_page.items);
}
```

### Backward Navigation

**⚠️ Work in Progress** - Referenced in [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md)

Moving backward through pages (useful for "Previous Page" functionality):

```rust
// Navigate backward
if let Some(prev_cursor) = page.prev_cursor {
    let prev_page = Post::all()
        .order_by(Post::FIELDS.created_at().desc())
        .paginate(10)
        .before(Some(prev_cursor))  // ⚠️ Not yet implemented
        .collect(&db)
        .await?;
}
```

## Web API Integration

### Cursor Serialization

**⚠️ Work in Progress** - Referenced in [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md)

For web APIs, cursors can be serialized to opaque string tokens:

```rust
// Serialize cursor for JSON response
let response = json!({
    "posts": page.items,
    "next_cursor": page.next_cursor.map(|c| c.encode()),  // ⚠️ Not yet implemented
    "prev_cursor": page.prev_cursor.map(|c| c.encode()),  // ⚠️ Not yet implemented
});

// Deserialize cursor from request
let cursor = Cursor::decode(&request.cursor)?;  // ⚠️ Not yet implemented
```

### REST API Example

A typical REST endpoint using Toasty pagination:

```rust
#[get("/posts?<cursor>&<limit>")]
async fn list_posts(
    db: &Db,
    cursor: Option<String>,
    limit: Option<usize>
) -> Result<Json<PostsResponse>> {
    let page_size = limit.unwrap_or(10).min(100); // Cap at 100

    let mut query = Post::all()
        .order_by(Post::FIELDS.created_at().desc())
        .paginate(page_size);

    // Apply cursor if provided
    if let Some(cursor_token) = cursor {
        let cursor = Cursor::decode(&cursor_token)?;  // ⚠️ Not yet implemented
        query = query.after(Some(cursor));
    }

    let page = query.collect(&db).await?;

    Ok(Json(PostsResponse {
        posts: page.items,
        next_cursor: page.next_cursor.map(|c| c.encode()),  // ⚠️ Not yet implemented
        prev_cursor: page.prev_cursor.map(|c| c.encode()),  // ⚠️ Not yet implemented
    }))
}
```

## Ordering Requirements

### Mandatory Ordering

Pagination requires an explicit `ORDER BY` clause to ensure consistent results:

```rust
// ✅ Correct - explicit ordering
let page = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;

// ❌ Will panic - no ordering specified
let page = Post::all()
    .paginate(10)  // Error: pagination requires ordering
    .collect(&db)
    .await?;
```

### Multi-Column Ordering

**⚠️ Work in Progress** - Referenced in [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md)

For complex sorting, you can order by multiple columns:

```rust
// Current: Manual OrderBy construction required
use toasty::stmt::OrderBy;

let order = OrderBy::from([
    Post::FIELDS.status().asc(),
    Post::FIELDS.created_at().desc(),
]);

let page = Post::all()
    .order_by(order)
    .paginate(10)
    .collect(&db)
    .await?;

// Future: Chain multiple order_by calls
let page = Post::all()
    .order_by(Post::FIELDS.status().asc())
    .then_by(Post::FIELDS.created_at().desc())  // ⚠️ Not yet implemented
    .paginate(10)
    .collect(&db)
    .await?;
```

## Performance Characteristics

### Why Cursor-Based?

Toasty uses cursor-based pagination (also called keyset pagination) instead of traditional `LIMIT/OFFSET` because:

1. **Consistent Performance**: O(log n) complexity regardless of page number
2. **Stable Results**: New records don't shift pagination boundaries
3. **Database Agnostic**: Works efficiently on NoSQL databases like DynamoDB
4. **Real-time Friendly**: Handles concurrent insertions gracefully

### How It Works

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

## Database-Specific Behavior

### SQL Databases

For SQL databases (PostgreSQL, MySQL, SQLite), Toasty generates efficient keyset queries with composite comparisons.

### DynamoDB

**⚠️ Work in Progress** - Referenced in [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md)

For DynamoDB, Toasty maps cursors to DynamoDB's native `LastEvaluatedKey` pagination:

```rust
// Toasty cursor seamlessly becomes DynamoDB ExclusiveStartKey
let page = User::all()
    .order_by(User::FIELDS.created_at().desc())  // Uses GSI if needed
    .paginate(10)
    .after(cursor)  // Becomes ExclusiveStartKey internally
    .collect(&db)
    .await?;
```

## Advanced Patterns

### Infinite Scroll

Common pattern for web applications:

```rust
async fn load_more_posts(
    db: &Db,
    last_cursor: Option<Cursor>
) -> Result<Page<Post>> {
    let mut query = Post::all()
        .order_by(Post::FIELDS.created_at().desc())
        .paginate(20);

    if let Some(cursor) = last_cursor {
        query = query.after(Some(cursor));
    }

    query.collect(db).await
}
```

### Page-Based UI

**⚠️ Work in Progress** - Referenced in [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md)

For traditional page number UIs, you'll need to maintain cursor state:

```rust
// Helper for page-based navigation
struct PageNavigator {
    cursors: Vec<Option<Cursor>>,  // Cursor for each page
    current_page: usize,
}

impl PageNavigator {
    pub async fn goto_page(&mut self, page_num: usize, db: &Db) -> Result<Page<Post>> {
        // Implementation would maintain cursor history
        // and navigate to requested page
    }
}
```

## Current Limitations

See [roadmap/order_limit_pagination.md](../roadmap/order_limit_pagination.md) for the complete list of features in progress:

- **Backward navigation** (`.before()` method)
- **Cursor serialization** (`.encode()`/`.decode()` methods)
- **Multi-column ordering** (`.then_by()` chaining)
- **DynamoDB pagination** (native `LastEvaluatedKey` support)
- **Page-based helpers** (traditional page number APIs)

## Best Practices

### 1. Always Include Unique Field in Ordering

Ensure deterministic ordering by including a unique field (usually ID) as a tie-breaker:

```rust
// ✅ Good - includes unique ID for tie-breaking
let page = Post::all()
    .order_by([
        Post::FIELDS.score().desc(),
        Post::FIELDS.id().asc(),  // Tie-breaker
    ])
    .paginate(10)
    .collect(&db)
    .await?;
```

### 2. Cap Page Sizes

Protect against abuse by limiting maximum page size:

```rust
let page_size = requested_size.min(100);  // Cap at 100 items
```

### 3. Handle Edge Cases

Check for empty results and missing cursors:

```rust
if page.items.is_empty() {
    return Ok(Json(EmptyResponse { message: "No more results" }));
}
```

### 4. Consider UX for Cursor Expiration

**⚠️ Work in Progress** - Future cursor tokens may include expiration

```rust
// Future: Handle expired cursors gracefully
match Cursor::decode(&token) {
    Ok(cursor) => query.after(Some(cursor)),
    Err(_) => query, // Start from beginning on invalid cursor
}
```