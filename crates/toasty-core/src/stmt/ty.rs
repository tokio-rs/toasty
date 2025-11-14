use super::{Id, PathFieldSet, TypeEnum, Value};
use crate::{
    schema::app::{FieldId, ModelId},
    stmt, Result,
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

    /// An opaque type that uniquely identifies an instance of a model.
    Id(ModelId),

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

    /// An enumeration of multiple types
    Enum(TypeEnum),

    /// The null type can be cast to any type.
    ///
    /// TODO: we should get rid of this.
    Null,

    SparseRecord(PathFieldSet),

    /// Unit type
    Unit,

    /// A type that could not be inferred (e.g., empty list)
    Unknown,
}

impl Type {
    pub fn list(ty: impl Into<Self>) -> Self {
        Self::List(Box::new(ty.into()))
    }

    #[track_caller]
    pub fn unwrap_list_ref(&self) -> &Type {
        match self {
            stmt::Type::List(items) => items,
            _ => todo!("expected stmt::Type::List; actual={self:#?}"),
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }

    pub fn is_id(&self) -> bool {
        matches!(self, Self::Id(_))
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(..))
    }

    pub fn cast(&self, value: Value) -> Result<Value> {
        use stmt::Value;

        // Null values are passed through
        if value.is_null() {
            return Ok(value);
        }

        Ok(match (value, self) {
            (value @ Value::String(_), Self::String) => value,
            (Value::Id(value), _) => value.cast(self)?,
            (Value::String(value), Self::Id(ty)) => Value::Id(Id::from_string(*ty, value)),
            (Value::Record(record), Self::SparseRecord(fields)) => {
                Value::sparse_record(fields.clone(), record)
            }
            (value, _) => todo!("value={value:#?}; ty={self:#?}"),
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
