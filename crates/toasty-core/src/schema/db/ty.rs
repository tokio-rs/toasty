use super::TypeEnum;
use crate::{Result, driver, stmt};

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    /// A boolean value
    Boolean,

    /// A signed integer of `n` bytes
    Integer(u8),

    /// An unsigned integer of `n` bytes
    UnsignedInteger(u8),

    /// A floating point number of `n` bytes
    Float(u8),

    /// Unconstrained text type
    Text,

    /// Text type with an explicit maximum length
    VarChar(u64),

    /// 128-bit universally unique identifier (UUID)
    Uuid,

    /// Decimal number with optional precision and scale.
    /// - `None`: Arbitrary-precision decimal
    /// - `Some((precision, scale))`: Fixed precision and scale
    Numeric(#[cfg_attr(feature = "serde", serde(with = "numeric_serde"))] Option<(u32, u32)>),

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

    /// A database enum type. See [`TypeEnum`].
    Enum(TypeEnum),

    /// An array of `T`, e.g. PostgreSQL `INT8[]` or `TEXT[]`. Used both for
    /// array column storage (where supported) and to type list-shaped bind
    /// parameters that the engine sends as a single PG array operand.
    List(Box<Type>),

    /// A document column storing a structured value as a single unit:
    /// `jsonb` / `json` on PostgreSQL, `JSON` on MySQL, JSON1 text on SQLite,
    /// BSON on MongoDB, a Map attribute on DynamoDB. `binary` selects the
    /// binary encoding (`jsonb`) over the text encoding (`json`) where the
    /// backend distinguishes them.
    Document {
        /// Selects the binary document encoding (`jsonb`) over the text
        /// encoding (`json`) where the backend distinguishes them.
        binary: bool,
    },

    /// A native SQL `JSON` column.
    Json,

    /// A native SQL `JSONB` column.
    Jsonb,

    /// User-specified unrecognized type
    Custom(String),
}

#[cfg(feature = "serde")]
mod numeric_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Values(Vec<u32>),
        Legacy(Option<(u32, u32)>),
    }

    pub fn serialize<S>(value: &Option<(u32, u32)>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some((precision, scale)) => [*precision, *scale].serialize(serializer),
            None => <[u32; 0]>::default().serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<(u32, u32)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Repr::deserialize(deserializer)? {
            Repr::Values(values) => match values.as_slice() {
                [] => Ok(None),
                [precision, scale] => Ok(Some((*precision, *scale))),
                _ => Err(D::Error::custom(
                    "numeric storage type requires zero or two parameters",
                )),
            },
            Repr::Legacy(value) => Ok(value),
        }
    }
}

impl Type {
    /// Construct a list storage type, collapsing a list of documents into a
    /// single document.
    ///
    /// A `#[document]` collection (`Vec<embed>`) is stored as one JSON blob,
    /// never a native array, so `db::Type` must never nest a [`Type::Document`]
    /// inside a [`Type::List`]. Routing list construction through here makes
    /// that invariant hold by construction: the list *is* the document.
    pub fn list(elem: Type) -> Type {
        match elem {
            Type::Document { binary } => Type::Document { binary },
            elem => Type::List(Box::new(elem)),
        }
    }

    /// The named enum this storage type carries: a scalar [`Type::Enum`] or the
    /// element of an enum array (`List(Enum)`).
    pub fn named_enum(&self) -> Option<&TypeEnum> {
        match self {
            Type::Enum(type_enum) => Some(type_enum),
            Type::List(elem) => match elem.as_ref() {
                Type::Enum(type_enum) => Some(type_enum),
                _ => None,
            },
            _ => None,
        }
    }

    /// Mutable counterpart to [`named_enum`](Self::named_enum).
    pub fn named_enum_mut(&mut self) -> Option<&mut TypeEnum> {
        match self {
            Type::Enum(type_enum) => Some(type_enum),
            Type::List(elem) => match elem.as_mut() {
                Type::Enum(type_enum) => Some(type_enum),
                _ => None,
            },
            _ => None,
        }
    }

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
                stmt::Type::F32 => Ok(Type::Float(4)),
                stmt::Type::F64 => Ok(Type::Float(8)),
                stmt::Type::String => Ok(db.default_string_type.clone()),
                stmt::Type::Uuid => Ok(db.default_uuid_type.clone()),
                stmt::Type::Bytes => Ok(db.default_bytes_type.clone()),
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
                // An embedded model column is stored as a single document
                // (`jsonb` on PG, `JSON` elsewhere) — never a native array.
                // `Type::list` collapses a list-of-documents back to one
                // document, so a `#[document]` collection (`List(Model)`) lands
                // here as a plain `Document`.
                stmt::Type::Model(_) => Ok(Type::Document { binary: true }),
                stmt::Type::List(elem) => Ok(Type::list(Self::from_app(elem, None, db)?)),
                _ => Err(crate::Error::unsupported_feature(format!(
                    "type {:?} is not supported by this database",
                    ty
                ))),
            },
        }
    }

    pub(crate) fn from_app_column(
        ty: &stmt::Type,
        hint: Option<&Type>,
        db: &driver::Capability,
        auto_increment: bool,
    ) -> Result<Type> {
        let mut storage_ty = Self::from_app(ty, hint, &db.storage_types)?;

        if auto_increment && let Some(max) = db.max_auto_increment_integer_width {
            match &mut storage_ty {
                Type::Integer(size) | Type::UnsignedInteger(size) if *size > max => {
                    *size = max;
                }
                _ => {}
            }
        }

        Ok(storage_ty)
    }

    /// Determines the [`stmt::Type`] closest to this [`db::Type`] that should be used
    /// as an intermediate conversion step to lessen the work done by each individual driver.
    pub fn bridge_type(&self, ty: &stmt::Type) -> stmt::Type {
        match (self, ty) {
            // Collections use the same application-to-storage conversion as
            // their elements. For example, an integer-discriminant enum is
            // `I64` in the application schema, while `#[column(type = u8)]`
            // stores `Vec<Enum>` as `List(U8)`.
            (Self::List(storage), stmt::Type::List(app)) => {
                stmt::Type::List(Box::new(storage.bridge_type(app)))
            }
            (Self::Blob | Self::Binary(_), stmt::Type::Uuid) => stmt::Type::Bytes,
            (Self::Text | Self::VarChar(_), _) => stmt::Type::String,
            // Enum values are always strings at the application level
            (Self::Enum(_), _) => stmt::Type::String,
            // Integer-discriminant enums use I64 at the application level.
            // An explicit storage width bridges through the corresponding
            // statement type so lowering inserts checked casts in both
            // directions.
            (Self::Integer(1), stmt::Type::I64) => stmt::Type::I8,
            (Self::Integer(2), stmt::Type::I64) => stmt::Type::I16,
            (Self::Integer(3..=4), stmt::Type::I64) => stmt::Type::I32,
            (Self::UnsignedInteger(1), stmt::Type::I64) => stmt::Type::U8,
            (Self::UnsignedInteger(2), stmt::Type::I64) => stmt::Type::U16,
            (Self::UnsignedInteger(3..=4), stmt::Type::I64) => stmt::Type::U32,
            (Self::UnsignedInteger(5..=8), stmt::Type::I64) => stmt::Type::U64,
            // Let engine handle UTC conversion
            #[cfg(feature = "jiff")]
            (Self::Timestamp(_) | Self::DateTime(_), stmt::Type::Zoned) => stmt::Type::Timestamp,
            // Bool key/index fields stored as 1-byte integer (e.g. DynamoDB N("1"/"0")).
            // The engine casts Bool <-> I8 transparently via encode_column /
            // map_table_column_to_model; the driver handles them as plain numbers.
            (Self::Integer(1), stmt::Type::Bool) => stmt::Type::I8,
            // A `#[document]` column stores a structural document: the column
            // is typed by `Type::Object` (mirroring `Value::Object`), not by
            // the embedded model. The model identity stays an app/engine
            // concept, carried by `mapping::Mapping::document_columns`. A
            // document collection (`List(Model)`, whose storage collapses to
            // one document) keeps its list shape with `Object` elements.
            (Self::Document { .. }, stmt::Type::Model(_)) => stmt::Type::Object,
            (Self::Document { .. }, stmt::Type::List(elem))
                if matches!(**elem, stmt::Type::Model(_)) =>
            {
                stmt::Type::List(Box::new(stmt::Type::Object))
            }
            _ => ty.clone(),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        match *self {
            Type::Json if !db.native_json => Err(crate::Error::unsupported_feature(format!(
                "JSON column type is not supported by {}",
                db.driver_name
            ))),
            Type::Jsonb if !db.native_jsonb => Err(crate::Error::unsupported_feature(format!(
                "JSONB column type is not supported by {}",
                db.driver_name
            ))),
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
