# Query Ordering, Limits & Pagination

> **📖 User Guide:** See [guide/pagination.md](../guide/pagination.md) for complete usage examples and API documentation.

## Current State Analysis

### ✅ What's Already Implemented

#### 1. **Ordering (ORDER BY)**
- Full AST support with `OrderBy` and `OrderByExpr` types
- Direction support (ASC/DESC) 
- SQL generation for ORDER BY clauses
- Type-safe field access via generated code
- Working examples in tests

**Current API:**
```rust
// Single column ordering
let users = User::all()
    .order_by(User::FIELDS.created_at().desc())
    .collect(&db)
    .await?;
```

#### 2. **Basic Limit**
- AST support for `Limit` struct
- SQL LIMIT generation (without OFFSET)
- Integration with query builder

**Current API:**
```rust
// Note: Currently no direct .limit() method exposed
// Limit is used internally by pagination
```

#### 3. **Cursor-based Pagination** 
- Sophisticated keyset pagination implementation
- Automatic query rewriting (converts to WHERE clauses)
- Database-agnostic approach
- Validation that ORDER BY matches cursor fields

**Current API:**
```rust
// Cursor-based pagination
let posts = Post::all()
    .order_by(Post::FIELDS.id().desc())
    .paginate(10)
    .after(last_post_id)
    .collect(&db)
    .await?;
```

### ❌ What's Missing for MVP

#### 1. **Page<T> Return Type**
Currently `.paginate().collect()` returns `Vec<T>`, but should return `Page<T>`:

**Current API:**
```rust
let posts: Vec<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;
```

**Target API:**
```rust
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub prev_cursor: Option<Cursor>,
}

// collect() now returns Page<T>
let page: Page<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;
    
// Access items via page.items
for post in &page.items {
    println!("{}", post.title);
}
```

#### 2. **Cursor Type and Serialization**
Need a `Cursor` type that can be serialized for web APIs:

**Needed API:**
```rust
pub struct Cursor {
    expr: stmt::Expr,
}

impl Cursor {
    pub fn encode(&self) -> String { ... }  // For web APIs
    pub fn decode(token: &str) -> Result<Self> { ... }
}
```

#### 3. **Backward Navigation**
Add `.before()` method for previous page functionality:

**Needed API:**
```rust
let prev_page = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .before(page.prev_cursor)  // Navigate backward
    .collect(&db)
    .await?;
```

#### 4. **Multi-column Ordering**
Currently requires manual `OrderBy` construction for multiple columns.

**Needed API:**
```rust
// Chain multiple order_by calls
let users = User::all()
    .order_by(User::FIELDS.status().asc())
    .then_by(User::FIELDS.created_at().desc())  // Convenience method
    .paginate(10)
    .collect(&db)
    .await?;
```

#### 5. **Direct Limit Method**
Currently `limit()` is not exposed for non-paginated queries.

**Needed API:**
```rust
// Get first N records (without pagination)
let recent_posts: Vec<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .limit(5)
    .collect(&db)
    .await?;
```

#### 6. **First/Last Convenience Methods**
Common convenience methods for getting single records.

**Needed API:**
```rust
// Get first/last record
let first_user: Option<User> = User::all()
    .order_by(User::FIELDS.created_at().asc())
    .first(&db)
    .await?;
```

## Implementation Roadmap

### Phase 1: Core Pagination Types (Priority: High)

#### 1.1 Create Cursor and Page Types
- **Files:** `toasty/src/stmt/cursor.rs`, `toasty/src/stmt/page.rs`
- **Task:** Create basic cursor and page structures
- **Complexity:** Low
```rust
// cursor.rs
pub struct Cursor {
    expr: stmt::Expr,
}

// page.rs  
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub prev_cursor: Option<Cursor>,
}
```

#### 1.2 Modify Paginate to Return Page<T>
- **File:** `toasty/src/stmt/paginate.rs`
- **Task:** Change `.collect()` to return `Page<T>` instead of `Vec<T>`
- **Complexity:** Medium
- **Breaking Change:** This modifies the existing API
```rust
impl<M: Model> Paginate<M> {
    pub async fn collect(&self, db: &Db) -> Result<Page<M>> {
        // Fetch limit+1 to determine next_cursor
        // Build prev_cursor from current position
        // Return Page with cursors
    }
}
```

### Phase 2: Navigation & Serialization (Priority: Medium)

#### 2.1 Add Backward Navigation  
- **File:** `toasty/src/stmt/paginate.rs`
- **Task:** Add `.before()` method and backward cursor logic
- **Complexity:** Medium

#### 2.2 Cursor Serialization
- **File:** `toasty/src/stmt/cursor.rs`
- **Task:** Add `.encode()` and `.decode()` methods for web APIs
- **Complexity:** Medium
```rust
impl Cursor {
    pub fn encode(&self) -> String {
        // Base64 encode the stmt::Expr for web transport
    }
    
    pub fn decode(token: &str) -> Result<Self> {
        // Deserialize from base64 token
    }
}
```

#### 2.3 Multi-column Ordering
- **File:** `toasty-codegen/src/expand/query.rs`
- **Task:** Add `.then_by()` method for chained ordering
- **Complexity:** Medium

### Phase 3: Convenience Features (Priority: Low)

#### 3.1 Direct Limit Method
- **File:** `toasty-codegen/src/expand/query.rs`
- **Task:** Generate `.limit()` method for non-paginated queries
- **Complexity:** Low

#### 3.2 First/Last Methods
- **File:** `toasty-codegen/src/expand/query.rs`  
- **Task:** Generate `.first()` and `.last()` convenience methods
- **Complexity:** Low

## Testing Requirements

### Unit Tests Needed
1. ✅ Basic ordering (exists: `one_model_sort_limit.rs`)
2. ❌ Multi-column ordering
3. ❌ Limit with offset
4. ❌ Page-based pagination
5. ✅ Cursor-based pagination (partial coverage)
6. ❌ Edge cases (empty results, beyond last page)

### Integration Tests Needed
1. Test across all database drivers (SQLite, MySQL, PostgreSQL, DynamoDB)
2. Test with different data types for ordering
3. Test pagination with complex queries (joins, filters)
4. Performance tests for large datasets

## Database-Specific Considerations

### SQL Databases
- **MySQL:** Uses `LIMIT n OFFSET m`
- **PostgreSQL:** Uses `LIMIT n OFFSET m` or `FETCH FIRST`
- **SQLite:** Uses `LIMIT n OFFSET m`

### NoSQL Databases
- **DynamoDB:** Limited ordering support, pagination via LastEvaluatedKey
- Consider maintaining current cursor-based approach for NoSQL

## Success Criteria

An MVP for ordering, limits, and pagination should:

1. ✅ Allow ordering by any model field (ascending/descending)
2. ✅ Support efficient cursor-based pagination (current keyset approach)
3. ❌ Return `Page<T>` struct with navigation cursors
4. ❌ Support backward navigation with `.before()` method
5. ❌ Provide cursor serialization for web APIs (`.encode()`/`.decode()`)
6. ❌ Support multi-column ordering convenience (`.then_by()`)
7. ❌ Include first/last convenience methods
8. ❌ Work consistently across all supported databases

## Next Steps

1. **Phase 1:** Implement `Cursor` and `Page<T>` types
2. **Phase 1:** Modify `.paginate().collect()` to return `Page<T>`
3. **Phase 2:** Add backward navigation and cursor serialization
4. **Phase 3:** Convenience methods and ergonomic improvements

**Decision Made:** Focus on cursor-based pagination exclusively rather than adding traditional offset/limit pagination, as it provides better performance and database consistency.