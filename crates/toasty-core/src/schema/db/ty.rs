use crate::{driver, stmt, Result};

/// Database-level storage types representing how values are stored in the target database.
///
/// `db::Type` represents the **external** types used by specific database systems
/// (PostgreSQL, MySQL, SQLite, DynamoDB, etc.) to store column values. These are the actual
/// storage types that appear in `CREATE TABLE` statements and database schemas.
///
/// # Type System Hierarchy
///
/// Toasty has two distinct type systems:
///
/// 1. **[`stmt::Type`](crate::stmt::Type)**: Application and query engine types (internal to Toasty)
///    - Represents Rust types: `I8`, `I16`, `String`, etc.
///    - Types of [`stmt::Value`] and [`stmt::Expr`]
///    - Used throughout Toasty's query processing at both application and engine levels
///
/// 2. **`db::Type`** (this type): Database storage types (external)
///    - Represents database column types: `Integer(n)`, `Text`, `VarChar(n)`, etc.
///    - External representation specific to the target database
///    - Specified in the schema, used by drivers
///
/// # Mapping from Application to Database Types
///
/// The mapping from [`stmt::Type`] to `db::Type` happens at the driver boundary:
///
/// ```text
/// stmt::Type::String  →  db::Type::Text         (default for most databases)
///                     →  db::Type::VarChar(255)  (if specified in schema)
///
/// stmt::Type::I64     →  db::Type::Integer(8)   (8-byte integer)
/// stmt::Type::I32     →  db::Type::Integer(4)   (4-byte integer)
/// stmt::Type::Bool    →  db::Type::Boolean
/// ```
///
/// See [`Type::from_app`] for the complete mapping logic.
///
/// # Schema Usage
///
/// Each column in the database schema ([`Column`](crate::schema::db::Column)) stores both:
/// - `column.ty: stmt::Type` - How Toasty views the column internally (application/engine type)
/// - `column.storage_ty: Option<db::Type>` - How the database stores it externally (storage type)
///
/// When `storage_ty` is `None`, the driver uses default mappings from `stmt::Type`.
/// When specified, it allows fine-grained control over storage (e.g., `VARCHAR(50)` vs `TEXT`).
///
/// # Database-Specific Behavior
///
/// Different databases support different storage types. The driver's capability
/// structure ([`driver::Capability`]) describes what types are available:
///
/// - **PostgreSQL**: Supports `Text`, `VarChar`, `Integer`, `Boolean`, etc.
/// - **SQLite**: Uses type affinity; most types map to `TEXT`, `INTEGER`, `REAL`, or `BLOB`
/// - **DynamoDB**: Uses NoSQL types like `S` (string), `N` (number), `BOOL`, etc.
///
/// # See Also
///
/// - [`stmt::Type`](crate::stmt::Type) - Application and query engine type system with detailed flow documentation
/// - [`Type::from_app`] - Mapping logic from statement types to database types
/// - [`Column`](crate::schema::db::Column) - Schema representation with both type systems
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// A boolean value
    Boolean,

    /// A signed integer of `n` bytes
    Integer(u8),

    /// An unsigned integer of `n` bytes
    UnsignedInteger(u8),

    /// Unconstrained text type
    Text,

    /// Text type with an explicit maximum length
    VarChar(u64),

    /// 128-bit universally unique identifier (UUID)
    Uuid,

    /// Decimal number with optional precision and scale.
    /// - `None`: Arbitrary-precision decimal
    /// - `Some((precision, scale))`: Fixed precision and scale
    Numeric(Option<(u32, u32)>),

    /// Unconstrained binary type
    Blob,

    /// Fixed-size binary type of `n` bytes
    Binary(u8),

    /// An instant in time with fractional seconds precision (0-9 digits).
    Timestamp(u8),

    /// A representation of a civil date in the Gregorian calendar.
    Date,

    /// A representation of civil "wall clock" time with fractional seconds precision (0-9 digits).
    Time(u8),

    /// A representation of a civil datetime in the Gregorian calendar with fractional seconds precision (0-9 digits).
    DateTime(u8),

    /// User-specified unrecognized type
    Custom(String),
}

impl Type {
    /// Maps an application-level type to a database-level storage type.
    pub fn from_app(
        ty: &stmt::Type,
        hint: Option<&Type>,
        db: &driver::StorageTypes,
    ) -> Result<Type> {
        match hint {
            Some(ty) => Ok(ty.clone()),
            None => match ty {
                stmt::Type::Bool => Ok(Type::Boolean),
                stmt::Type::I8 => Ok(Type::Integer(1)),
                stmt::Type::I16 => Ok(Type::Integer(2)),
                stmt::Type::I32 => Ok(Type::Integer(4)),
                stmt::Type::I64 => Ok(Type::Integer(8)),
                // Map unsigned types to UnsignedInteger with appropriate byte width
                stmt::Type::U8 => Ok(Type::UnsignedInteger(1)),
                stmt::Type::U16 => Ok(Type::UnsignedInteger(2)),
                stmt::Type::U32 => Ok(Type::UnsignedInteger(4)),
                stmt::Type::U64 => Ok(Type::UnsignedInteger(8)),
                stmt::Type::String => Ok(db.default_string_type.clone()),
                stmt::Type::Uuid => Ok(db.default_uuid_type.clone()),
                // Decimal type
                #[cfg(feature = "rust_decimal")]
                stmt::Type::Decimal => Ok(db.default_decimal_type.clone()),
                // BigDecimal type
                #[cfg(feature = "bigdecimal")]
                stmt::Type::BigDecimal => Ok(db.default_bigdecimal_type.clone()),
                // Date/time types from jiff
                #[cfg(feature = "jiff")]
                stmt::Type::Timestamp => Ok(db.default_timestamp_type.clone()),
                #[cfg(feature = "jiff")]
                stmt::Type::Zoned => Ok(db.default_zoned_type.clone()),
                #[cfg(feature = "jiff")]
                stmt::Type::Date => Ok(db.default_date_type.clone()),
                #[cfg(feature = "jiff")]
                stmt::Type::Time => Ok(db.default_time_type.clone()),
                #[cfg(feature = "jiff")]
                stmt::Type::DateTime => Ok(db.default_datetime_type.clone()),
                // Gotta support some app-level types as well for now.
                //
                // TODO: not really correct, but we are getting rid of ID types
                // most likely.
                stmt::Type::Id(_) => Ok(db.default_string_type.clone()),
                // Enum types are stored as strings in the database
                stmt::Type::Enum(_) => Ok(db.default_string_type.clone()),
                _ => Err(crate::Error::unsupported_feature(format!(
                    "type {:?} is not supported by this database",
                    ty
                ))),
            },
        }
    }

    /// Determines the [`stmt::Type`] closest to this [`db::Type`] that should be used
    /// as an intermediate conversion step to lessen the work done by each individual driver.
    pub fn bridge_type(&self, ty: &stmt::Type) -> stmt::Type {
        match (self, ty) {
            (Self::Blob | Self::Binary(_), stmt::Type::Uuid) => stmt::Type::Bytes,
            (Self::Text | Self::VarChar(_), _) => stmt::Type::String,
            // Let engine handle UTC conversion
            #[cfg(feature = "jiff")]
            (Self::Timestamp(_) | Self::DateTime(_), stmt::Type::Zoned) => stmt::Type::Timestamp,
            _ => ty.clone(),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        match *self {
            Type::VarChar(size) => match db.storage_types.varchar {
                Some(max) if size > max => Err(crate::Error::unsupported_feature(format!(
                    "VARCHAR({}) exceeds database maximum of {}",
                    size, max
                ))),
                None => Err(crate::Error::unsupported_feature(
                    "VARCHAR type is not supported by this database",
                )),
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }
}
