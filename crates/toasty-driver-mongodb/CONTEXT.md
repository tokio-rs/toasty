# toasty-driver-mongodb Component Context

## Purpose
MongoDB-specific driver implementation that translates abstract Toasty operations into MongoDB commands. Implements the common `Driver` trait while handling MongoDB-specific features as a NoSQL document database.

## Architecture

### Core Implementation
The driver follows the standard Toasty driver pattern:
1. Implements `driver::Driver` trait from toasty-core
2. Manages MongoDB connections via the official async Rust driver
3. Translates operations to MongoDB queries and commands
4. Converts between Toasty values and BSON documents
5. Handles MongoDB-specific features (document model, indexes, ObjectId)

### Connection Management
- **Connection String**: Standard MongoDB connection strings (`mongodb://host:port/database`)
- **Client**: Async MongoDB client from `mongodb` crate (v3.x)
- **Database**: Database instance extracted from connection URL path
- **Collections**: One collection per Toasty table

## MongoDB-Specific Features

### Document Model
- **Collections**: Each Toasty table maps to a MongoDB collection
- **Documents**: Records stored as BSON documents
- **Primary Keys**: Mapped to MongoDB's `_id` field
- **Flexible Schema**: MongoDB's schema-less nature allows for dynamic fields

### Type System
MongoDB BSON type mappings:
- **String** → String
- **I8, I16, I32** → Int32
- **I64** → Int64
- **U8, U16, U32, U64** → Int64 (with range checks for u64)
- **Bool** → Boolean
- **Uuid** → String (formatted as UUID string)
- **Id** → ObjectId (when valid) or String
- **Enum** → Document with `{variant: Int32, fields: Array}`
- **Record** → Array
- **SparseRecord** → Array with Null for missing values
- **List** → Array (TODO: full implementation)

### Index Support
- **Primary Key**: Automatically uses `_id` index
- **Unique Indexes**: Created with `unique: true` option
- **Compound Indexes**: Multiple fields in single index
- **Secondary Indexes**: Support for non-unique indexes
- **Auto-Creation**: Indexes created during `register_schema()`

### Transaction Support
- **Status**: Structured but not yet fully implemented
- **Requirement**: MongoDB replica sets or sharded clusters
- **Implementation Plan**:
  - `Transaction::Start` → Create ClientSession and start transaction
  - `Transaction::Commit` → Commit transaction on session
  - `Transaction::Rollback` → Abort transaction on session
- **Note**: Requires session state management in driver

## Operations Implementation

### Insert
```rust
// Single document
collection.insert_one(document).await
// Batch insert
collection.insert_many(documents).await
```
- Maps table columns to document fields
- Uses `_id` for primary key field
- Skips null values (MongoDB handles missing fields gracefully)
- Returns count of inserted documents

### GetByKey
```rust
// Single key
collection.find_one(filter).with_options(projection).await
// Multiple keys
collection.find({_id: {$in: [keys]}}).await
```
- Single key lookup for one document
- `$in` operator for multiple keys
- Projection for selecting specific fields
- Returns streaming results via ValueStream

### UpdateByKey
```rust
collection.update_one(filter, {$set: updates}).await
// or
collection.update_many(filter, {$set: updates}).await
```
- Uses `$set` operator for field updates
- Single or batch updates based on key count
- TODO: Filter and condition expression conversion
- Returns count of modified documents

### DeleteByKey
```rust
collection.delete_one(filter).await
// or
collection.delete_many(filter).await
```
- Single or batch deletes
- Filter by `_id` field
- Returns count of deleted documents

### QueryPk
```rust
collection.find(filter).with_options(projection).await
```
- Primary key filtering
- Projection for field selection
- TODO: Complex expression conversion to MongoDB query DSL
- Returns streaming results

### FindPkByIndex
```rust
collection.find(index_filter).with_options({projection: {_id: 1}}).await
```
- Queries using secondary indexes
- Returns only primary key values
- Projection limits results to `_id` field only
- TODO: Complex filter expression support

### QuerySql
- Handles statements that originated as SQL
- Currently delegates INSERT to Insert operation
- TODO: Support for other statement types

### Transaction
- Structure in place for MongoDB session-based transactions
- TODO: Implement session creation and management
- TODO: Store active sessions in driver state
- Requires MongoDB 4.0+ with replica set configuration

## Value Conversion

### BSON → Toasty (from_bson)
Located in `src/value.rs`:

```rust
pub fn from_bson(bson: &Bson, ty: &stmt::Type) -> stmt::Value
```

Handles:
- Type-directed conversion based on schema
- ObjectId → Id conversion
- String → UUID parsing
- Proper null handling
- Type safety through stmt::Type matching

### Toasty → BSON (to_bson)
```rust
pub fn to_bson(value: &stmt::Value) -> Bson
```

Handles:
- ObjectId generation for valid ID strings
- Enum serialization as `{variant, fields}` document
- Record and array conversions
- Null value handling
- U64 overflow handling (converts to string if > i64::MAX)

## Capabilities

MongoDB advertises these capabilities:
```rust
Capability::MONGODB = Capability {
    sql: false,                      // Document-oriented, not SQL
    storage_types: StorageTypes::MONGODB,
    cte_with_update: false,          // N/A for NoSQL
    select_for_update: false,        // Uses transactions instead
    primary_key_ne_predicate: true,  // Supports != on _id
}

StorageTypes::MONGODB = StorageTypes {
    default_string_type: db::Type::Text,  // No varchar limits
    varchar: None,                         // MongoDB has no varchar
    max_unsigned_integer: None,            // Full u64 via BSON Int64
}
```

## Limitations & TODOs

### Current Limitations
1. **Expression Conversion**: Basic expressions only
   - Simple value comparisons work
   - Complex binary operations → TODO
   - Logical operators (AND, OR, NOT) → TODO

2. **Transactions**: Not yet implemented
   - Session management needed
   - Requires replica set or sharded cluster
   - State management for active sessions

3. **Aggregation Pipeline**: Not yet implemented
   - Would enable complex queries
   - GROUP BY, JOIN-like operations
   - Advanced filtering and transformations

4. **MongoDB-Specific Features**: Not yet implemented
   - Text search indexes
   - Geospatial queries
   - Array operations (`$elemMatch`, `$all`)
   - Change streams

### Expression Conversion TODO
Need to implement conversion from `stmt::Expr` to MongoDB query DSL:
- **BinaryOp**: `{field: {$eq, $ne, $gt, $gte, $lt, $lte}}`
- **LogicalOp**: `{$and: [...], $or: [...], $not: {...}}`
- **InList**: `{field: {$in: [...]}}`
- **Like**: `{field: {$regex: "..."}}`

## Testing

### Connection String Format
```rust
// Local MongoDB
"mongodb://localhost:27017/toasty_test"

// With authentication
"mongodb://user:pass@localhost:27017/toasty_test"

// Replica set (required for transactions)
"mongodb://localhost:27017,localhost:27018,localhost:27019/toasty_test?replicaSet=rs0"
```

### Test Patterns
- Use test database with unique name per test run
- Clean up collections after tests (or use unique collection names)
- Test with actual MongoDB instance (Docker recommended)
- Test both single and batch operations
- Verify index creation

### Docker Setup for Testing
```bash
# Start MongoDB
docker run -d -p 27017:27017 --name mongo-test mongo:7

# For transaction testing (replica set)
docker run -d -p 27017:27017 --name mongo-test \
  mongo:7 --replSet rs0

# Initialize replica set
docker exec mongo-test mongosh --eval "rs.initiate()"
```

## Common Change Patterns

### Adding a New Type
1. Add BSON conversion in `value.rs::to_bson()`
2. Add reverse conversion in `value.rs::from_bson()`
3. Handle NULL case appropriately
4. Test with edge cases and boundary values

### Optimizing Queries
1. Ensure indexes are created for frequently queried fields
2. Use projections to limit returned fields
3. Consider aggregation pipeline for complex queries
4. Use `$in` for batch lookups instead of multiple queries

### Implementing Expression Conversion
1. Add pattern matching in `build_filter_document()`
2. Map Toasty operators to MongoDB query operators
3. Handle nested expressions recursively
4. Test with various expression combinations

## Performance Considerations

### Best Practices
- **Indexes**: MongoDB performance heavily depends on proper indexing
- **Projections**: Always use projections to limit returned data
- **Batch Operations**: Use `insert_many`, `update_many` for bulk ops
- **Connection Pooling**: MongoDB driver handles connection pooling automatically
- **Document Size**: MongoDB documents limited to 16MB

### Optimization Opportunities
1. **Covered Queries**: Design indexes that can satisfy queries without reading documents
2. **Cursor Batching**: MongoDB driver handles this automatically
3. **Write Concerns**: Can be tuned for performance vs. durability
4. **Read Concerns**: Can be configured based on consistency requirements

## Recent Changes

From initial development:
- Implemented all 8 core operations
- Added automatic index creation from schema
- Primary key mapping to `_id` field
- BSON value conversion system
- Projection support for efficient queries
- Streaming results via ValueStream
- Proper error conversion from MongoDB errors

## Related Components

- **toasty-core**: Defines operations this driver implements
- **toasty**: Engine that calls this driver
- **mongodb**: Official MongoDB Rust driver (async)
- **bson**: BSON serialization/deserialization
- Other drivers (DynamoDB, SQLite, PostgreSQL): Reference implementations

## Next Steps

1. **Expression Conversion**: Full support for complex filter expressions
2. **Transactions**: Implement session-based transaction management
3. **Aggregation Pipeline**: Support for complex queries and grouping
4. **Integration Tests**: Comprehensive test suite with real MongoDB
5. **Performance Testing**: Benchmarks and optimization
6. **Advanced Features**: Text search, geospatial, array operations
