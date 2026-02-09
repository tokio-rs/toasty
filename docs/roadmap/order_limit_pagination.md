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

### ✅ Recently Implemented

#### 1. **Page<T> Return Type**
`.paginate().collect()` now returns `Page<T>`:

**Current API:**
```rust
let page: Page<Post> = Post::all()
    .order_by(Post::FIELDS.created_at().desc())
    .paginate(10)
    .collect(&db)
    .await?;

// Access items via page.items or deref
for post in &page.items {
    println!("{}", post.title);
}
```

#### 2. **Page Navigation Methods**
Navigate forward and backward through pages:

**Current API:**
```rust
// Forward navigation
if let Some(next_page) = page.next(&db).await? {
    process_posts(&next_page.items);
}

// Backward navigation
if let Some(prev_page) = page.prev(&db).await? {
    process_posts(&prev_page.items);
}
```

#### 3. **First Convenience Method**
Get the first matching record:

**Current API:**
```rust
let first_user: Option<User> = User::all()
    .order_by(User::FIELDS.created_at().asc())
    .first(&db)
    .await?;
```

### ❌ What's Missing for MVP

#### 1. **Cursor Serialization**
Need serialization methods for web APIs:

**Needed API:**
```rust
// Serialize cursor for JSON response
let next_token = page.next_cursor.map(|c| c.encode());

// Deserialize cursor from request
let cursor = Cursor::decode(&token)?;
```

#### 2. **Multi-column Ordering Convenience**
Currently requires manual `OrderBy` construction for multiple columns.

**Current Workaround:**
```rust
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
```

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

#### 3. **Direct Limit Method**
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

#### 4. **Last Convenience Method**
Get the last matching record:

**Needed API:**
```rust
let last_user: Option<User> = User::all()
    .order_by(User::FIELDS.created_at().desc())
    .last(&db)
    .await?;
```

## Implementation Roadmap

### ✅ Completed Phases

#### Phase 1: Core Pagination Types
- ✅ Created `Page<T>` type in `toasty/src/page.rs`
- ✅ Modified `.paginate().collect()` to return `Page<T>`
- ✅ Added `.next()` and `.prev()` methods on `Page<T>`
- ✅ Implemented backward navigation with `.before()` in `toasty/src/stmt/paginate.rs`

#### Phase 2: First Convenience Method
- ✅ Generated `.first()` method in `toasty-codegen/src/expand/query.rs`

### 🚧 Remaining Work

#### Phase 3: Cursor Serialization (Priority: High)
- **Task:** Add `.encode()` and `.decode()` methods for web APIs
- **Complexity:** Medium
- **Files:** New file or methods on `stmt::Expr`

```rust
// Option 1: Add methods to stmt::Expr
impl stmt::Expr {
    pub fn encode(&self) -> String {
        // Base64 encode the expression for web transport
    }

    pub fn decode(token: &str) -> Result<Self> {
        // Deserialize from base64 token
    }
}

// Option 2: Create a wrapper Cursor type
pub struct Cursor(stmt::Expr);

impl Cursor {
    pub fn encode(&self) -> String { ... }
    pub fn decode(token: &str) -> Result<Self> { ... }
}
```

#### Phase 4: Multi-column Ordering (Priority: Medium)
- **File:** `toasty-codegen/src/expand/query.rs`
- **Task:** Add `.then_by()` method for chained ordering
- **Complexity:** Medium

#### Phase 5: Convenience Features (Priority: Low)

##### 5.1 Direct Limit Method
- **File:** `toasty-codegen/src/expand/query.rs`
- **Task:** Generate `.limit()` method for non-paginated queries
- **Complexity:** Low

##### 5.2 Last Method
- **File:** `toasty-codegen/src/expand/query.rs`
- **Task:** Generate `.last()` convenience method
- **Complexity:** Low

## Testing Requirements

### ✅ Existing Tests
1. ✅ Basic ordering - `crates/toasty-driver-integration-suite/src/tests/one_model_sort_limit.rs::sort_asc`
2. ✅ Page-based pagination - `one_model_sort_limit.rs::paginate`
3. ✅ Forward navigation (`.after()` and `page.next()`)
4. ✅ Backward navigation (`.prev()`)
5. ✅ First convenience method - `one_model_crud.rs` and others

### ❌ Tests Still Needed
1. ❌ Multi-column ordering
2. ❌ Cursor serialization (`.encode()`/`.decode()`)
3. ❌ Direct `.limit()` method (non-paginated queries)
4. ❌ `.last()` convenience method
5. ❌ Edge cases:
   - Empty results with pagination
   - Single page results (no next/prev cursors)
   - Pagination beyond last page

## Database-Specific Considerations

### SQL Databases
- **MySQL:** Uses `LIMIT n OFFSET m`
- **PostgreSQL:** Uses `LIMIT n OFFSET m` or `FETCH FIRST`
- **SQLite:** Uses `LIMIT n OFFSET m`

### NoSQL Databases
- **DynamoDB:** Limited ordering support, pagination via LastEvaluatedKey
- Consider maintaining current cursor-based approach for NoSQL

## Success Criteria

Current status of MVP features:

1. ✅ Allow ordering by any model field (ascending/descending)
2. ✅ Support efficient cursor-based pagination (keyset approach)
3. ✅ Return `Page<T>` struct with navigation cursors
4. ✅ Support backward navigation with `.before()` method and `page.prev()`
5. ✅ Include `.first()` convenience method
6. ✅ Work consistently across SQL databases (SQLite, MySQL, PostgreSQL)
7. ❌ Provide cursor serialization for web APIs (`.encode()`/`.decode()`)
8. ❌ Support multi-column ordering convenience (`.then_by()`)
9. ❌ Include `.last()` convenience method
10. ❌ Include direct `.limit()` method for non-paginated queries
11. ⚠️ DynamoDB support (partial - needs validation)

## Next Steps

Priority order for remaining work:

1. **High Priority:** Cursor serialization (Phase 3)
   - Essential for web APIs
   - Enables stateless pagination
   - Blocks many real-world use cases

2. **Medium Priority:** Multi-column ordering convenience (Phase 4)
   - Improves ergonomics
   - Workaround exists (manual `OrderBy` construction)

3. **Low Priority:** Additional convenience methods (Phase 5)
   - `.limit()` for non-paginated queries
   - `.last()` convenience method
   - Nice-to-have features

**Note:** The core pagination infrastructure is complete and production-ready for applications that can manage cursors internally. Web API support requires cursor serialization.