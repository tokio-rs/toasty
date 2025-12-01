# MongoDB Example for Toasty

This example demonstrates using Toasty ORM with MongoDB.

## Prerequisites

1. MongoDB running locally or accessible via connection string
2. Default connection: `mongodb://localhost:27017/toasty_example`

### Quick Start with Docker

```bash
# Start MongoDB
docker run -d -p 27017:27017 --name mongodb-toasty mongo:7

# Stop and remove
docker stop mongodb-toasty && docker rm mongodb-toasty
```

## Running the Example

```bash
# With default connection (localhost)
cargo run --example mongodb-example

# With custom connection string
TOASTY_CONNECTION_URL=mongodb://localhost:27017/mydb cargo run --example mongodb-example

# With authentication
TOASTY_CONNECTION_URL=mongodb://user:pass@host:port/dbname cargo run --example mongodb-example
```

## What This Example Demonstrates

- Creating models with `#[derive(toasty::Model)]`
- Connecting to MongoDB via connection string
- CRUD operations (Create, Read, Update, Delete)
- Unique indexes (email field)
- Secondary indexes (user_id field)
- Relationships (HasMany, BelongsTo)
- Querying by ID and unique fields
- Streaming results

## Model Structure

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key] #[auto] id: Id<Self>,
    name: String,
    #[unique] email: String,
    #[has_many] posts: HasMany<Post>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key] #[auto] id: Id<Self>,
    #[index] user_id: Id<User>,
    #[belongs_to(key = user_id, references = id)] user: BelongsTo<User>,
    title: String,
    content: String,
}
```

## MongoDB Features Used

- Collections: `users` and `posts`
- Primary keys mapped to `_id` field (ObjectId)
- Unique index on `users.email`
- Secondary index on `posts.user_id`
- Document storage for model data
- Efficient projections for queries
