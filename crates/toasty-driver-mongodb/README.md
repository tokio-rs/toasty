# toasty-driver-mongodb

MongoDB driver implementation for [Toasty](https://github.com/tokio-rs/toasty) ORM.

## Features

- ✅ Full CRUD operations (Create, Read, Update, Delete)
- ✅ Automatic index creation from schema
- ✅ Primary key mapping to MongoDB `_id`
- ✅ Batch operations (insert_many, update_many, delete_many)
- ✅ Efficient projections and field selection
- ✅ Streaming query results
- ✅ BSON type conversion
- ⏳ Transaction support (structured, implementation pending)
- ⏳ Complex expression filters (basic support, advanced pending)
- ⏳ Aggregation pipeline (planned)

## Usage

```rust
use toasty_driver_mongodb::MongoDb;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to MongoDB
    let driver = MongoDb::connect("mongodb://localhost:27017/my_database").await?;

    // Use with Toasty ORM
    // let db = toasty::Db::new(driver);

    Ok(())
}
```

## Connection Strings

Standard MongoDB connection string format:

```
mongodb://localhost:27017/database_name
mongodb://user:password@localhost:27017/database_name
mongodb://host1:27017,host2:27017/database_name?replicaSet=rs0
```

## Type Mappings

| Toasty Type | MongoDB/BSON Type |
|-------------|-------------------|
| String      | String            |
| I32         | Int32             |
| I64         | Int64             |
| Bool        | Boolean           |
| Uuid        | String            |
| Id          | ObjectId or String|
| Enum        | Document          |
| Record      | Array             |

## Requirements

- MongoDB 4.0+ (for full feature support)
- MongoDB 4.0+ with Replica Set (for transactions)
- Rust 1.70+

## Testing

Start a local MongoDB instance:

```bash
# Using Docker
docker run -d -p 27017:27017 --name mongodb-test mongo:7

# Or with docker-compose
docker-compose up -d mongodb
```

Run tests:

```bash
cargo test --features mongodb
```

## Performance Tips

1. **Indexes**: Create indexes on frequently queried fields
2. **Projections**: Use field selection to limit data transfer
3. **Batch Operations**: Use batch inserts/updates for better performance
4. **Connection Pooling**: MongoDB driver handles this automatically

## Limitations

Current implementation has these limitations:

- Transaction support requires session management (TODO)
- Complex filter expressions not yet fully supported
- Aggregation pipeline not yet implemented
- Some MongoDB-specific features not yet exposed (text search, geospatial)

See `CONTEXT.md` for detailed information about architecture and future enhancements.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
