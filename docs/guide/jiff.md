# Date and Time Types with Jiff

Toasty supports date and time types through the [`jiff`](https://github.com/BurntSushi/jiff) crate, which provides high-quality temporal types for Rust.

## Enabling Jiff Support

To use jiff types in your models, enable the `jiff` feature on the `toasty` crate:

```toml
[dependencies]
toasty = { version = "...", features = ["jiff"] }
jiff = "..."
```

The `jiff` feature is enabled by default in `toasty`.

## Supported Types

Toasty supports five jiff temporal types:

### 1. Timestamp - Instant in Time

`jiff::Timestamp` represents an instant in time (number of nanoseconds since the Unix epoch). This is the recommended type for storing UTC timestamps.

```rust
use jiff::Timestamp;

#[derive(toasty::Model)]
struct Event {
    #[key]
    id: Id<Self>,
    created_at: Timestamp,
}
```

**Database Storage:**
- **PostgreSQL**: `TIMESTAMPTZ(6)` - with microsecond precision
- **MySQL**: `DATETIME(6)` - with microsecond precision, stored as UTC
- **SQLite**: `TEXT` - ISO 8601 format with full nanosecond precision
- **DynamoDB**: `TEXT` - ISO 8601 format with full nanosecond precision

### 2. Zoned - Timezone-Aware Instant

`jiff::Zoned` represents a timezone-aware instant in time.

```rust
use jiff::Zoned;

#[derive(toasty::Model)]
struct Appointment {
    #[key]
    id: Id<Self>,
    scheduled_at: Zoned,
}
```

**Database Storage:**
- **PostgreSQL**: `TEXT` - to preserve timezone
- **MySQL**: `TEXT` - to preserve timezone
- **SQLite**: `TEXT`
- **DynamoDB**: `TEXT`

### 3. Date - Civil Date

`jiff::civil::Date` represents a calendar date without time or timezone information.

```rust
use jiff::civil::Date;

#[derive(toasty::Model)]
struct Person {
    #[key]
    id: Id<Self>,
    birth_date: Date,
}
```

**Database Storage:**
- **PostgreSQL**: `DATE`
- **MySQL**: `DATE`
- **SQLite**: `TEXT` - ISO 8601 format (YYYY-MM-DD)
- **DynamoDB**: `TEXT` - ISO 8601 format

### 4. Time - Wall Clock Time

`jiff::civil::Time` represents a time of day without date or timezone information.

```rust
use jiff::civil::Time;

#[derive(toasty::Model)]
struct Schedule {
    #[key]
    id: Id<Self>,
    daily_reminder: Time,
}
```

**Database Storage:**
- **PostgreSQL**: `TIME(6)` - with microsecond precision
- **MySQL**: `TIME(6)` - with microsecond precision
- **SQLite**: `TEXT` - ISO 8601 format with full nanosecond precision
- **DynamoDB**: `TEXT` - ISO 8601 format with full nanosecond precision

### 5. DateTime - Civil DateTime

`jiff::civil::DateTime` represents a calendar date and time without timezone information.

```rust
use jiff::civil::DateTime;

#[derive(toasty::Model)]
struct Meeting {
    #[key]
    id: Id<Self>,
    local_time: DateTime,
}
```

**Database Storage:**
- **PostgreSQL**: `TIMESTAMP(6)` - with microsecond precision
- **MySQL**: `DATETIME(6)` - with microsecond precision
- **SQLite**: `TEXT` - ISO 8601 format with full nanosecond precision
- **DynamoDB**: `TEXT` - ISO 8601 format with full nanosecond precision

## Precision Limitations

### PostgreSQL and MySQL

PostgreSQL and MySQL support temporal types with a maximum precision of **6 decimal places (microseconds)**. When storing jiff values (which support nanosecond precision), the fractional seconds are truncated to microseconds:

```rust
use jiff::civil::Time;

// This value has nanosecond precision
let time = Time::constant(14, 30, 45, 123_456_789);
// 123_456_789 nanoseconds

// After storing in PostgreSQL/MySQL and reading back:
// 123_456_000 nanoseconds (truncated to microseconds)
```

### Custom Precision

You can specify a different precision using the `#[column]` attribute:

```rust
use jiff::Timestamp;

#[derive(toasty::Model)]
struct Event {
    #[key]
    id: Id<Self>,

    // Store with 2 decimal places (centiseconds)
    #[column(type = timestamp(2))]
    created_at: Timestamp,
}
```

Valid precision values are 0-6 for PostgreSQL and MySQL. Higher precision values will cause a database error.

### Storing as Text for Full Precision

If you need to preserve full nanosecond precision on PostgreSQL or MySQL, you can override the storage type to use TEXT:

```rust
use jiff::Timestamp;

#[derive(toasty::Model)]
struct HighPrecisionEvent {
    #[key]
    id: Id<Self>,

    // Store as TEXT to preserve nanosecond precision
    #[column(type = text)]
    created_at: Timestamp,
}
```

When stored as TEXT, jiff values are serialized using ISO 8601 format with full nanosecond precision. This works on all databases and is the default storage method for SQLite and DynamoDB.

## Database-Specific Behavior

### SQLite

SQLite does not have native date/time types. All temporal values are automatically stored as TEXT in ISO 8601 format with full nanosecond precision. This means:

- No precision is lost when round-tripping values
- Values are human-readable in the database
- The `#[column(type = ...)]` attribute has no effect for temporal types on SQLite

### DynamoDB

Like SQLite, DynamoDB stores all temporal values as TEXT (string type) in ISO 8601 format with full nanosecond precision.

### MySQL TIMESTAMP Limitations

MySQL's `TIMESTAMP` type only supports dates from 1970-01-01 00:00:01 UTC to 2038-01-19 03:14:07 UTC. To avoid these limitations, Toasty uses `DATETIME` as the default storage type for `Timestamp` and `Zoned` on MySQL, which supports a much wider range (1000-01-01 to 9999-12-31).

## Best Practices

1. **Use `Timestamp` for UTC timestamps**: If you're storing points in time, use `jiff::Timestamp` rather than `Zoned` unless you specifically need timezone-aware operations in your application code.

2. **Be aware of precision loss**: When using PostgreSQL or MySQL with their native types, remember that nanosecond precision is truncated to microsecond precision.

3. **Consider TEXT storage for portability**: If you need your schema to work identically across all databases, consider using `#[column(type = text)]` for temporal types. This ensures consistent behavior and full precision everywhere.

## Example: Complete Model

```rust
use jiff::{Timestamp, Zoned};
use jiff::civil::{Date, Time, DateTime};

#[derive(toasty::Model)]
struct Event {
    #[key]
    #[auto]
    id: Id<Self>,

    // Standard timestamp (microsecond precision on PostgreSQL/MySQL)
    created_at: Timestamp,

    // Timezone-aware
    scheduled_at: Zoned,

    // Event date (no time component)
    event_date: Date,

    // Daily recurring time
    reminder_time: Time,

    // Local datetime without timezone
    local_datetime: DateTime,

    // High precision timestamp stored as text
    #[column(type = text)]
    high_precision_timestamp: Timestamp,

    // Custom precision (2 decimal places)
    #[column(type = timestamp(2))]
    low_precision_timestamp: Timestamp,
}
```
