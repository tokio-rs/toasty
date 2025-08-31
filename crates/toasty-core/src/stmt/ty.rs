use super::{Id, PathFieldSet, TypeEnum, Value};
use crate::{
    schema::app::{FieldId, ModelId},
    stmt, Result,
};

/// An expression type.
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
    Null,

    SparseRecord(PathFieldSet),
}

impl Type {
    pub fn list(ty: impl Into<Self>) -> Self {
        Self::List(Box::new(ty.into()))
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

    /// Returns true if the type contains no model-level types (after lowering).
    /// Model-level types are: Id(ModelId), Key(ModelId), Model(ModelId), ForeignKey(FieldId)
    pub fn is_lowered(&self) -> bool {
        match self {
            // Model-level types - these should not exist after lowering
            Self::Id(_) | Self::Key(_) | Self::Model(_) | Self::ForeignKey(_) => false,

            // Primitive types - these are fine after lowering
            Self::Bool
            | Self::String
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
            | Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::Null => true,

            // Composite types - recursively check their contents
            Self::List(inner) => inner.is_lowered(),
            Self::Record(fields) => fields.iter().all(|field| field.is_lowered()),
            Self::Enum(type_enum) => type_enum
                .variants
                .iter()
                .all(|variant| variant.fields.iter().all(|field| field.is_lowered())),
            Self::SparseRecord(_) => true, // SparseRecord is a table-level construct
        }
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
