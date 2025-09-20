# toasty-driver-sqlite Component Context

## Purpose
SQLite-specific driver implementation that translates abstract Toasty operations into SQLite commands. Implements the common `Driver` trait while handling SQLite-specific features and limitations.

## Architecture

### Core Implementation
The driver follows the standard pattern:
1. Implements `driver::Driver` trait from toasty-core
2. Manages SQLite connections (file-based and in-memory)
3. Translates operations to SQL statements
4. Converts between Toasty values and SQLite types
5. Handles SQLite-specific features/limitations

### Connection Management
- **In-memory databases**: Supports `:memory:` for testing
- **File-based databases**: Standard file paths with proper locking
- **Tempfile support**: Automatic cleanup for test databases
- **Connection string parsing**: URL-based configuration
- **Synchronous wrapper**: SQLite operations wrapped in async runtime

## SQLite-Specific Features

### Type System
- **INTEGER PRIMARY KEY AUTOINCREMENT**: For auto-incrementing IDs
- **TEXT**: String storage
- **REAL**: Floating-point numbers
- **BLOB**: Binary data
- **INTEGER**: Boolean as 0/1, all integer types

### Transaction Support
- Basic transaction support with BEGIN/COMMIT/ROLLBACK
- Savepoint support for nested transactions
- Automatic rollback on errors
- Deferred transaction mode by default

### Prepared Statements
- Statement caching for performance
- Parameter binding with proper escaping
- Reusable statements across queries

## Operations Implementation

### GetByKey
```sql
SELECT * FROM table WHERE id = ?
```
- Single row fetch
- Uses prepared statements
- Returns Option<Value>

### Insert
```sql
INSERT INTO table (columns...) VALUES (?, ?, ...)
```
- Uses `last_insert_rowid()` for auto-increment retrieval
- Returns inserted row with generated ID

### Update
```sql
UPDATE table SET column = ? WHERE id = ?
```
- Affects single or multiple rows
- Returns number of affected rows

### Delete
```sql
DELETE FROM table WHERE id = ?
```
- Simple deletion by primary key
- Returns success/failure status

### Query
- Full SELECT support with WHERE, ORDER BY, LIMIT, OFFSET
- JOIN operations supported
- Subquery support
- Stream-based result handling

## Value Conversion

Located in the driver module, handling conversions:

### Toasty → SQLite
- `Value::String` → TEXT
- `Value::I64` → INTEGER
- `Value::F64` → REAL
- `Value::Bool` → INTEGER (0/1)
- `Value::Bytes` → BLOB
- `Value::Null` → NULL

### SQLite → Toasty
- TEXT → `Value::String`
- INTEGER → `Value::I64` or `Value::Bool`
- REAL → `Value::F64`
- BLOB → `Value::Bytes`
- NULL → `Value::Null`

## Capabilities

SQLite advertises these capabilities:
```rust
Capability {
    auto_increment: true,      // via AUTOINCREMENT
    returning: false,           // Not supported natively
    transactions: true,         // Full transaction support
    joins: true,               // Full JOIN support
    subqueries: true,          // Subquery support
    indexes: true,             // Index support
}
```

## Limitations

### SQLite-Specific Constraints
- No native RETURNING clause (emulated with last_insert_rowid)
- Limited concurrent write access (single writer)
- No native boolean type (uses INTEGER)
- Case-insensitive LIKE by default
- Limited ALTER TABLE capabilities

### Performance Considerations
- File I/O bound for disk databases
- Write-ahead logging (WAL) mode for better concurrency
- Index usage critical for query performance
- VACUUM periodically for space reclamation

## Testing

### Test Database Setup
```rust
// In-memory for fast tests
"sqlite::memory:"

// Tempfile for isolation
let tmpfile = tempfile::NamedTempFile::new()?;
format!("sqlite://{}", tmpfile.path().display())
```

### Test Patterns
- Use in-memory databases for unit tests
- Tempfiles for integration tests requiring persistence
- Automatic cleanup via Drop trait
- Test both memory and file-based modes

## Common Change Patterns

### Adding a New Type
1. Map type in value conversion
2. Choose appropriate SQLite storage type
3. Handle in parameter binding
4. Handle in result extraction
5. Test with NULL and edge cases

### Optimizing Queries
1. Ensure indexes are used (EXPLAIN QUERY PLAN)
2. Use prepared statements
3. Batch operations in transactions
4. Consider WAL mode for concurrent reads

### Debugging
1. Enable query logging
2. Use EXPLAIN to understand query plans
3. Check for lock contention
4. Monitor prepared statement cache

## Recent Changes

From recent development:
- Improved error handling for connection failures
- Better tempfile management in tests
- Reduced glob imports for cleaner code
- Transaction savepoint support
- Prepared statement cache optimization

## Related Components

- **toasty-sql**: Generates SQL statements for this driver
- **toasty-core**: Defines operations this driver implements
- **toasty**: Engine that calls this driver
- Other drivers: Can reference for common patterns, but each has unique characteristics