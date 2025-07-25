use super::*;

/// An expression type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Boolean value
    Bool,

    /// String type
    String,

    /// Signed 32-bit integer
    I32,

    /// Signed 64-bit integer
    I64,

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

    pub fn casts_to(&self, other: &Self) -> bool {
        match self {
            Self::Null => true,
            Self::List(item) => match other {
                Self::List(other_item) => item.casts_to(other_item),
                // A list of 1 item can be flattened when cast. Right now, we
                // can't statically know if a list will only have 1 item, so we
                // just say it can cast.
                _ => item.casts_to(other),
            },
            Self::Record(items) => match other {
                Self::Record(other_items) => items
                    .iter()
                    .zip(other_items.iter())
                    .all(|(item, other_item)| item.casts_to(other_item)),
                _ => false,
            },
            Self::Id(model) | Self::Model(model) => match other {
                Self::Id(other_model) => model == other_model,
                Self::Model(other_model) => model == other_model,
                _ => false,
            },
            _ => self == other,
        }
    }

    pub fn applies_binary_op(&self, op: BinaryOp) -> bool {
        use BinaryOp::*;
        use Type::*;

        match op {
            Eq | Ne => match self {
                Bool | String | I32 | I64 | Id(_) | Key(_) | Model(_) | ForeignKey(_) => true,
                Null => false,
                List(ty) => ty.applies_binary_op(op),
                Record(tys) => tys.iter().all(|ty| ty.applies_binary_op(op)),
                Enum(_) | SparseRecord(_) => todo!(),
            },
            Ge | Gt | Le | Lt => match self {
                I32 | I64 => true,
                Bool | String | Id(_) | Key(_) | Model(_) | ForeignKey(_) | Null | List(_)
                | Record(_) | Enum(_) | SparseRecord(_) => false,
            },
            _ => todo!("op = {:#?}", op),
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
