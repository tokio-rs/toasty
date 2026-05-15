use super::{PathFieldSet, TypeDocument, TypeUnion, Value};
use crate::{
    Result,
    schema::app::{FieldId, ModelId},
    stmt,
};

/// Statement-level type system for values and expressions within Toasty's query engine.
///
/// `stmt::Type` represents types at both the **application level** (models, fields, Rust types)
/// and the **query engine level** (tables, columns, internal processing). These types are
/// **internal to Toasty** - they describe how Toasty views and processes data throughout the
/// entire query pipeline, from user queries to driver execution.
///
/// # Distinction from Database Types
///
/// Toasty has two distinct type systems:
///
/// 1. **`stmt::Type`** (this type): Application and query engine types
///    - Types of [`stmt::Value`] and [`stmt::Expr`] throughout query processing
///    - Represents Rust primitive types: `I8`, `I16`, `String`, etc.
///    - Works at both model level (application) and table/column level (engine)
///    - Internal to Toasty's query processing pipeline
///
/// 2. **[`schema::db::Type`](crate::schema::db::Type)**: Database storage types
///    - External representation for the target database
///    - Database-specific types: `Integer(n)`, `Text`, `VarChar(n)`, etc.
///    - Used only at the driver boundary when generating database queries
///
/// The key distinction: `stmt::Type` is how **Toasty** views types internally, while
/// [`schema::db::Type`](crate::schema::db::Type) is how the **database** stores them externally.
///
/// # Query Processing Pipeline
///
/// Throughout query processing, all values and expressions are typed using `stmt::Type`,
/// even as they are transformed and converted:
///
/// **Application Level (Model/Field)**
/// - User writes queries referencing models and fields
/// - Types like `stmt::Type::Model(UserId)`, `stmt::Type::String`
/// - Values like `stmt::Value::String("alice")`, `stmt::Value::I64(42)`
///
/// **Query Engine Level (Table/Column)**
/// - During planning, queries are "lowered" from models to tables
/// - Values may be converted between types (e.g., Model → Record, Id → String)
/// - All conversions are from `stmt::Type` to `stmt::Type`
/// - Still using the same type system, now at table/column abstraction level
///
/// **Driver Boundary (Database Storage)**
/// - Statements with `stmt::Value` (typed by `stmt::Type`) passed to drivers
/// - Driver consults schema to map `stmt::Type` → [`schema::db::Type`](crate::schema::db::Type)
/// - Same `stmt::Type::String` may map to different database types based on schema configuration
///
/// # Schema Representation
///
/// Each column in the database schema stores both type representations:
/// - `column.ty: stmt::Type` - How Toasty views this column internally
/// - `column.storage_ty: Option<db::Type>` - How the database stores it externally
///
/// This dual representation enables flexible mapping. For instance, `stmt::Type::String`
/// might map to `db::Type::Text` in one column and `db::Type::VarChar(100)` in another,
/// depending on schema configuration and database capabilities.
///
/// # See Also
///
/// - [`schema::db::Type`](crate::schema::db::Type) External database storage types
/// - [`stmt::Value`] - Values typed by this system
/// - [`stmt::Expr`] - Expressions typed by this system
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    /// Boolean value
    Bool,

    /// String type
    String,

    /// Signed 8-bit integer
    I8,

    /// Signed 16-bit integer
    I16,

    /// Signed 32-bit integer
    I32,

    /// Signed 64-bit integer
    I64,

    /// Unsigned 8-bit integer
    U8,

    /// Unsigned 16-bit integer
    U16,

    /// Unsigned 32-bit integer
    U32,

    /// Unsigned 64-bit integer
    U64,

    /// 32-bit floating point number
    F32,

    /// 64-bit floating point number
    F64,

    /// 128-bit universally unique identifier (UUID)
    Uuid,

    /// An instance of a model key
    Key(ModelId),

    /// An instance of a model
    Model(ModelId),

    /// An instance of a foreign key for a specific relation
    ForeignKey(FieldId),

    /// A list of a single type
    List(Box<Type>),

    /// A fixed-length tuple where each item can have a different type.
    Record(Vec<Type>),

    /// A document: a named, ordered set of fields, stored as a single value
    /// (JSON, BSON, ...). The named counterpart to [`Type::Record`].
    Document(TypeDocument),

    /// A byte array, more efficient than `List(U8)`.
    Bytes,

    /// A fixed-precision decimal number.
    /// See [`rust_decimal::Decimal`].
    #[cfg(feature = "rust_decimal")]
    Decimal,

    /// An arbitrary-precision decimal number.
    /// See [`bigdecimal::BigDecimal`].
    #[cfg(feature = "bigdecimal")]
    BigDecimal,

    /// An instant in time represented as the number of nanoseconds since the Unix epoch.
    /// See [`jiff::Timestamp`].
    #[cfg(feature = "jiff")]
    Timestamp,

    /// A time zone aware instant in time.
    /// See [`jiff::Zoned`]
    #[cfg(feature = "jiff")]
    Zoned,

    /// A representation of a civil date in the Gregorian calendar.
    /// See [`jiff::civil::Date`].
    #[cfg(feature = "jiff")]
    Date,

    /// A representation of civil “wall clock” time.
    /// See [`jiff::civil::Time`].
    #[cfg(feature = "jiff")]
    Time,

    /// A representation of a civil datetime in the Gregorian calendar.
    /// See [`jiff::civil::DateTime`].
    #[cfg(feature = "jiff")]
    DateTime,

    /// The null type. Represents the type of a null value and is cast-able to
    /// any type. Also used as the element type of an empty list whose item type
    /// is not yet known.
    Null,

    /// A record type where only a subset of fields are populated, identified
    /// by a [`PathFieldSet`].
    SparseRecord(PathFieldSet),

    /// Unit type
    Unit,

    /// A type that could not be inferred (e.g., empty list)
    Unknown,

    /// A union of possible types.
    ///
    /// Used when a match expression's arms can produce values of different types
    /// (e.g., a mixed enum where unit arms return `I64` and data arms return
    /// `Record`). A value is compatible with a union if it satisfies any of the
    /// member types.
    Union(TypeUnion),
}

impl Type {
    /// Creates a [`Type::List`] wrapping the given element type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::Type;
    /// let ty = Type::list(Type::String);
    /// assert!(ty.is_list());
    /// ```
    pub fn list(ty: impl Into<Self>) -> Self {
        Self::List(Box::new(ty.into()))
    }

    /// Returns the element type of this list type, panicking if this is not
    /// a [`Type::List`].
    ///
    /// # Panics
    ///
    /// Panics if the type is not a `List` variant.
    #[track_caller]
    pub fn as_list_unwrap(&self) -> &Type {
        match self {
            stmt::Type::List(items) => items,
            _ => panic!("expected stmt::Type::List; actual={self:#?}"),
        }
    }

    /// Returns `true` if this is [`Type::Bool`].
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }

    /// Returns `true` if this is [`Type::Model`].
    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }

    /// Returns `true` if this is [`Type::List`].
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// Returns `true` if this is [`Type::String`].
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    /// Returns `true` if this is [`Type::Unit`].
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    /// Returns `true` if this is [`Type::Record`].
    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(..))
    }

    /// Returns `true` if this is [`Type::Document`].
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Document(..))
    }

    /// Returns `true` if this is [`Type::Bytes`].
    pub fn is_bytes(&self) -> bool {
        matches!(self, Self::Bytes)
    }

    /// Returns `true` if this is [`Type::Decimal`] (requires `rust_decimal` feature).
    pub fn is_decimal(&self) -> bool {
        #[cfg(feature = "rust_decimal")]
        {
            matches!(self, Self::Decimal)
        }
        #[cfg(not(feature = "rust_decimal"))]
        {
            false
        }
    }

    /// Returns `true` if this is [`Type::BigDecimal`] (requires `bigdecimal` feature).
    pub fn is_big_decimal(&self) -> bool {
        #[cfg(feature = "bigdecimal")]
        {
            matches!(self, Self::BigDecimal)
        }
        #[cfg(not(feature = "bigdecimal"))]
        {
            false
        }
    }

    /// Returns `true` if this is [`Type::Uuid`].
    pub fn is_uuid(&self) -> bool {
        matches!(self, Self::Uuid)
    }

    /// Returns `true` if this is [`Type::SparseRecord`].
    pub fn is_sparse_record(&self) -> bool {
        matches!(self, Self::SparseRecord(..))
    }

    /// Returns `true` if this type is a numeric integer type.
    ///
    /// Numeric types include all signed and unsigned integer types:
    /// `I8`, `I16`, `I32`, `I64`, `U8`, `U16`, `U32`, `U64`.
    ///
    /// This does not include decimal types or floating-point types.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::Type;
    /// assert!(Type::I32.is_numeric());
    /// assert!(Type::U64.is_numeric());
    /// assert!(!Type::String.is_numeric());
    /// assert!(!Type::Bool.is_numeric());
    /// ```
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
        )
    }

    /// Casts `value` to this type, returning the converted value.
    ///
    /// Null values pass through unchanged. Supported conversions include
    /// identity casts, string/UUID interchange, string/decimal interchange,
    /// record-to-sparse-record, and integer width conversions.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion is not supported or if the value
    /// is out of range for the target type.
    pub fn cast(&self, value: Value) -> Result<Value> {
        use stmt::Value;

        // Null values are passed through
        if value.is_null() {
            return Ok(value);
        }

        #[cfg(feature = "jiff")]
        if let Some(value) = self.cast_jiff(&value)? {
            return Ok(value);
        }

        Ok(match (value, self) {
            // Identity
            (value @ Value::String(_), Self::String) => value,
            // String <-> Uuid
            (Value::Uuid(value), Self::String) => Value::String(value.to_string()),
            (Value::String(value), Self::Uuid) => {
                Value::Uuid(value.parse().expect("could not parse uuid"))
            }
            // Bytes <-> Uuid
            (Value::Uuid(value), Self::Bytes) => Value::Bytes(value.as_bytes().to_vec()),
            (Value::Bytes(value), Self::Uuid) => {
                let bytes = value.clone();
                Value::Uuid(
                    value
                        .try_into()
                        .map_err(|_| crate::Error::type_conversion(Value::Bytes(bytes), "Uuid"))?,
                )
            }
            // String <-> Decimal
            #[cfg(feature = "rust_decimal")]
            (Value::Decimal(value), Self::String) => Value::String(value.to_string()),
            #[cfg(feature = "rust_decimal")]
            (Value::String(value), Self::Decimal) => {
                Value::Decimal(value.parse().expect("could not parse Decimal"))
            }
            // String <-> BigDecimal
            #[cfg(feature = "bigdecimal")]
            (Value::BigDecimal(value), Self::String) => Value::String(value.to_string()),
            #[cfg(feature = "bigdecimal")]
            (Value::String(value), Self::BigDecimal) => {
                Value::BigDecimal(value.parse().expect("could not parse BigDecimal"))
            }
            // Record <-> SparseRecord
            (Value::Record(record), Self::SparseRecord(fields)) => {
                Value::sparse_record(fields.clone(), record)
            }
            // Integer conversions - use TryFrom which provides error messages
            (value, Self::I8) => Value::I8(i8::try_from(value)?),
            (value, Self::I16) => Value::I16(i16::try_from(value)?),
            (value, Self::I32) => Value::I32(i32::try_from(value)?),
            (value, Self::I64) => Value::I64(i64::try_from(value)?),
            (value, Self::U8) => Value::U8(u8::try_from(value)?),
            (value, Self::U16) => Value::U16(u16::try_from(value)?),
            (value, Self::U32) => Value::U32(u32::try_from(value)?),
            (value, Self::U64) => Value::U64(u64::try_from(value)?),
            // Float casts
            (Value::F32(v), Self::F32) => Value::F32(v),
            (Value::F64(v), Self::F32) => {
                let converted = v as f32;
                if converted.is_infinite() && !v.is_infinite() {
                    return Err(crate::Error::type_conversion(
                        Value::F64(v),
                        "f32 (overflow)",
                    ));
                }
                Value::F32(converted)
            }
            (Value::F32(v), Self::F64) => Value::F64(v as f64),
            (Value::F64(v), Self::F64) => Value::F64(v),
            (value, _) => todo!("value={value:#?}; ty={self:#?}"),
        })
    }

    /// Checks whether `self` (the actual/inferred type) is assignable to `other`
    /// (the expected type).
    ///
    /// This is a subtype check, NOT strict equality:
    /// - [`Type::Null`] matches any type (in either direction), since it represents
    ///   "we don't know what type this is"
    /// - A concrete type is assignable to a [`Type::Union`] if it matches any member
    /// - A [`Type::Union`] is assignable to another union if every member of `self`
    ///   matches some member of `other`
    /// - Container types ([`Type::List`], [`Type::Record`]) check element/field
    ///   types recursively
    ///
    /// # Examples
    ///
    /// - `String.is_subtype_of(String)` -> true
    /// - `String.is_subtype_of(Null)` -> true
    /// - `String.is_subtype_of(Bytes)` -> false
    /// - `Record([...]).is_subtype_of(Union([I64, Record([...])]))` -> true
    /// - `I64.is_subtype_of(Union([I64, Record(...)]))` -> true
    /// - `String.is_subtype_of(Union([I64, Record(...)]))` -> false
    pub fn is_subtype_of(&self, other: &Type) -> bool {
        // Null matches anything (commutative)
        if matches!(self, Type::Null) || matches!(other, Type::Null) {
            return true;
        }

        match (self, other) {
            // Simple types must match exactly
            (Type::Bool, Type::Bool) => true,
            (Type::String, Type::String) => true,
            (Type::I8, Type::I8) => true,
            (Type::I16, Type::I16) => true,
            (Type::I32, Type::I32) => true,
            (Type::I64, Type::I64) => true,
            (Type::U8, Type::U8) => true,
            (Type::U16, Type::U16) => true,
            (Type::U32, Type::U32) => true,
            (Type::U64, Type::U64) => true,
            (Type::F32, Type::F32) => true,
            (Type::F64, Type::F64) => true,
            (Type::Uuid, Type::Uuid) => true,
            (Type::Bytes, Type::Bytes) => true,
            (Type::Unit, Type::Unit) => true,
            (Type::Unknown, Type::Unknown) => true,

            // Decimal types
            #[cfg(feature = "rust_decimal")]
            (Type::Decimal, Type::Decimal) => true,
            #[cfg(feature = "bigdecimal")]
            (Type::BigDecimal, Type::BigDecimal) => true,

            // Temporal types
            #[cfg(feature = "jiff")]
            (Type::Timestamp, Type::Timestamp) => true,
            #[cfg(feature = "jiff")]
            (Type::Zoned, Type::Zoned) => true,
            #[cfg(feature = "jiff")]
            (Type::Date, Type::Date) => true,
            #[cfg(feature = "jiff")]
            (Type::Time, Type::Time) => true,
            #[cfg(feature = "jiff")]
            (Type::DateTime, Type::DateTime) => true,

            // Model-related types must match model IDs
            (Type::Key(a), Type::Key(b)) => a == b,
            (Type::Model(a), Type::Model(b)) => a == b,
            (Type::ForeignKey(a), Type::ForeignKey(b)) => a == b,

            // List types: element type must be assignable
            (Type::List(a), Type::List(b)) => a.is_subtype_of(b),

            // Record types: same length and all fields recursively assignable
            (Type::Record(a), Type::Record(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.is_subtype_of(b))
            }

            // Document types: same field names (in order) and recursively
            // assignable field types.
            (Type::Document(a), Type::Document(b)) => {
                a.fields.len() == b.fields.len()
                    && a.fields
                        .iter()
                        .zip(b.fields.iter())
                        .all(|(a, b)| a.name == b.name && a.ty.is_subtype_of(&b.ty))
            }

            // A positional record is assignable to a document type when the
            // field types line up — `Value::Record` is the positional form
            // of a document value, and drivers decode document columns
            // straight to records.
            (Type::Record(a), Type::Document(b)) => {
                a.len() == b.fields.len()
                    && a.iter()
                        .zip(b.fields.iter())
                        .all(|(a, b)| a.is_subtype_of(&b.ty))
            }

            // Sparse records must have the same field set
            (Type::SparseRecord(a), Type::SparseRecord(b)) => a == b,

            // Union-to-Union: every member of self must be assignable to some member of other
            (Type::Union(a), Type::Union(b)) => a
                .iter()
                .all(|a_ty| b.iter().any(|b_ty| a_ty.is_subtype_of(b_ty))),

            // Concrete type assignable to union if it matches any member
            (ty, Type::Union(union)) => union.iter().any(|member| ty.is_subtype_of(member)),

            // Union assignable to concrete type if every member is assignable
            (Type::Union(union), other) => union.iter().all(|member| member.is_subtype_of(other)),

            // Different type variants are not assignable
            _ => false,
        }
    }
}

impl From<&Self> for Type {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl From<ModelId> for Type {
    fn from(value: ModelId) -> Self {
        Self::Model(value)
    }
}
