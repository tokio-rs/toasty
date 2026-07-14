use super::{PathFieldSet, Resolve, TypeUnion, Value, ValueObject, ValueRecord};
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

    /// A document value with named fields — the type-level mirror of
    /// [`Value::Object`](super::Value::Object).
    ///
    /// This is how a `#[document]` column is typed at the database and driver
    /// level: purely structural, like a `jsonb` column. It does not name the
    /// embedded model whose fields it stores — that identity is an app/engine
    /// concept, and the engine views the same column as [`Type::Model`]. The
    /// two views are converted at the driver boundary (see the engine's
    /// document lowering and raising).
    Object,

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

    /// Returns `true` if this is [`Type::Object`].
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object)
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

    /// Whether this type has a document position (`Type::Model`).
    ///
    /// Values at a document position convert between the engine's positional
    /// records and the named objects drivers consume; such conversions are
    /// schema-directed and cannot run in a schema-free context.
    pub fn contains_model(&self) -> bool {
        match self {
            Self::Model(_) => true,
            Self::List(elem) => elem.contains_model(),
            Self::Record(fields) => fields.iter().any(Self::contains_model),
            Self::Union(union) => union.iter().any(|ty| ty.contains_model()),
            _ => false,
        }
    }

    /// Casts `value` to this type, returning the converted value.
    ///
    /// Null values pass through unchanged. Supported conversions include
    /// identity casts, string/UUID interchange, string/decimal interchange,
    /// record-to-sparse-record, integer width conversions, and — directed by
    /// `resolve` — raising a `#[document]` position's named wire object into
    /// the embedded model's positional record.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion is not supported, if the value
    /// is out of range for the target type, or if a schema-directed
    /// conversion cannot resolve its model through `resolve`.
    pub fn cast(&self, resolve: &impl Resolve, value: Value) -> Result<Value> {
        self.cast_from(resolve, None, value)
    }

    /// Casts `value` to this type, additionally directed by the source type
    /// when one is known (see [`super::ExprCast::from`]).
    ///
    /// A model-level `from` type triggers the document *lowering* conversion:
    /// the engine's positional record becomes the named object drivers
    /// consume. Every other conversion is directed by the target type alone,
    /// exactly as [`Self::cast`].
    pub fn cast_from(
        &self,
        resolve: &impl Resolve,
        from: Option<&Type>,
        value: Value,
    ) -> Result<Value> {
        use stmt::Value;

        // Null values are passed through
        if value.is_null() {
            return Ok(value);
        }

        // Lowering: a `#[document]` position converts from the engine's
        // positional form to the named object drivers consume, directed by
        // the *source* type — the structural target does not name the embed
        // and a positional record is not self-describing.
        if let Some(from) = from
            && from.contains_model()
        {
            return Self::lower_document(resolve, from, value);
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
            // Bool <-> I8: Bool key/index fields are stored as Integer(1) via
            // bridge_type. The engine casts Bool -> I8 on write and I8 -> Bool
            // on read. Only Type::cast supports this; TryFrom is intentionally
            // kept strict so raw numeric conversions don't silently accept Bool.
            (Value::Bool(v), Self::I8) => Value::I8(if v { 1 } else { 0 }),
            (Value::I8(v), Self::Bool) => Value::Bool(v != 0),
            // Integer conversions - use TryFrom which provides error messages
            (value, Self::I8) => Value::I8(i8::try_from(value)?),
            (value, Self::I16) => Value::I16(i16::try_from(value)?),
            (value, Self::I32) => Value::I32(i32::try_from(value)?),
            (value, Self::I64) => Value::I64(i64::try_from(value)?),
            (value, Self::U8) => Value::U8(u8::try_from(value)?),
            (value, Self::U16) => Value::U16(u16::try_from(value)?),
            (value, Self::U32) => Value::U32(u32::try_from(value)?),
            (value, Self::U64) => Value::U64(u64::try_from(value)?),
            // Integer -> float conversions. Document leaves decode from the
            // wire by integer fit (an integral JSON number or DynamoDB `N`
            // arrives as `I64`/`U64`), so raising a float document field must
            // accept integer-shaped input.
            (Value::I64(v), Self::F32) => Value::F32(v as f32),
            (Value::I64(v), Self::F64) => Value::F64(v as f64),
            (Value::U64(v), Self::F32) => Value::F32(v as f32),
            (Value::U64(v), Self::F64) => Value::F64(v as f64),
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
            // Raising: a named wire object at a document position becomes the
            // embedded model's positional record; engine-computed values
            // already in positional form pass through.
            (value, Self::Model(_)) => return self.raise_document(resolve, value),
            (Value::List(items), Self::List(elem)) => Value::List(
                items
                    .into_iter()
                    .map(|item| elem.cast(resolve, item))
                    .collect::<Result<_>>()?,
            ),
            (Value::Record(record), Self::Record(fields)) if fields.len() == record.len() => {
                Value::Record(ValueRecord::from_vec(
                    fields
                        .iter()
                        .zip(record)
                        .map(|(ty, value)| ty.cast(resolve, value))
                        .collect::<Result<_>>()?,
                ))
            }
            // A union member is picked by shape: cast with the first member
            // the value satisfies (a wire object satisfies its `Type::Model`
            // member via the named field check).
            (value, Self::Union(union)) => match union.iter().find(|ty| value.is_a(resolve, ty)) {
                Some(ty) => return ty.cast(resolve, value),
                None => value,
            },
            (value, _) => todo!("value={value:#?}; ty={self:#?}"),
        })
    }

    /// Raise a value at a document position: a named wire object (the form a
    /// driver decodes shape-directed) becomes the embedded model's positional
    /// record, in schema field order. A key the writer omitted decodes to
    /// `Null`; a key unknown to the schema (written by an external client) is
    /// dropped. A value already in engine form (an engine-computed positional
    /// record) passes through, so the conversion is idempotent.
    fn raise_document(&self, resolve: &impl Resolve, value: Value) -> Result<Value> {
        let Self::Model(embed_id) = self else {
            panic!("raise_document on non-model type; ty={self:#?}")
        };

        // Already in engine form — idempotence for engine-computed values.
        let Value::Object(object) = value else {
            return Ok(value);
        };

        let Some(model) = resolve.model(*embed_id) else {
            return Err(crate::Error::expression_evaluation_failed(format!(
                "cannot cast to {self:?}: the model is not resolvable in this context"
            )));
        };

        let mut entries = object.entries;
        Ok(Value::Record(ValueRecord::from_vec(
            model
                .fields()
                .iter()
                .map(|field| {
                    let name = field.name().app_unwrap();
                    match entries.iter().position(|(key, _)| key == name) {
                        Some(index) => field
                            .expr_ty()
                            .cast_document_leaf(resolve, entries.swap_remove(index).1),
                        None => Ok(Value::Null),
                    }
                })
                .collect::<Result<_>>()?,
        )))
    }

    /// Raise one document-interior value: descend document structure, pass
    /// through leaves already of the field's type, and cast the rest — the
    /// wire shapes a shape-directed decode produces (integers by fit,
    /// temporals / decimals / uuids as text) back to the field's type.
    fn cast_document_leaf(&self, resolve: &impl Resolve, value: Value) -> Result<Value> {
        match (self, value) {
            (Self::Model(_), value @ Value::Object(_)) => self.raise_document(resolve, value),
            (Self::List(elem), Value::List(items)) => Ok(Value::List(
                items
                    .into_iter()
                    .map(|item| elem.cast_document_leaf(resolve, item))
                    .collect::<Result<_>>()?,
            )),
            (_, Value::Null) => Ok(Value::Null),
            (ty, value) if value.is_a(resolve, ty) => Ok(value),
            (ty, value) => ty.cast(resolve, value),
        }
    }

    /// Lower a document value from the engine's positional form to the named
    /// object a driver serializes, directed by the model-level source type —
    /// the inverse of [`Self::raise_document`]. A `Type::Model` position turns
    /// its `Value::Record` into a `Value::Object`, resolving the embed's field
    /// names from the schema and recursing; `List` maps elementwise; anything
    /// else — including an already-named `Value::Object` — passes through, so
    /// the conversion is idempotent.
    fn lower_document(resolve: &impl Resolve, from: &Type, value: Value) -> Result<Value> {
        Ok(match (from, value) {
            (Type::Model(embed_id), Value::Record(record)) => {
                let Some(model) = resolve.model(*embed_id) else {
                    return Err(crate::Error::expression_evaluation_failed(format!(
                        "cannot cast from {from:?}: the model is not resolvable in this context"
                    )));
                };

                Value::Object(ValueObject::from_vec(
                    model
                        .fields()
                        .iter()
                        .zip(record)
                        .map(|(field, value)| {
                            Ok((
                                field.name().app_unwrap().to_owned(),
                                Self::lower_document(resolve, field.expr_ty(), value)?,
                            ))
                        })
                        .collect::<Result<_>>()?,
                ))
            }
            (Type::List(elem), Value::List(items)) => Value::List(
                items
                    .into_iter()
                    .map(|item| Self::lower_document(resolve, elem, item))
                    .collect::<Result<_>>()?,
            ),
            (_, value) => value,
        })
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
