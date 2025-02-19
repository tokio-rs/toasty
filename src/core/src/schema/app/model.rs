pub(super) mod attr;

mod from_ast;

mod index;
pub use index::{ModelIndex, ModelIndexField, ModelIndexId};

mod pk;
pub use pk::PrimaryKey;

use super::*;

use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Model {
    /// Uniquely identifies the model within the schema
    pub id: ModelId,

    /// Name of the model
    pub name: Name,

    /// Fields contained by the model
    pub fields: Vec<Field>,

    /// References the index that represents the model's primary key. This must
    /// be a unique index.
    pub primary_key: PrimaryKey,

    /// Prepared queries that query this model
    pub queries: Vec<QueryId>,

    pub indices: Vec<ModelIndex>,

    /// If the schema specifies a table to map the model to, this is set.
    pub table_name: Option<String>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ModelId(pub usize);

impl Model {
    pub fn primitives_mut(&mut self) -> impl Iterator<Item = &mut FieldPrimitive> + '_ {
        self.fields
            .iter_mut()
            .flat_map(|field| match &mut field.ty {
                FieldTy::Primitive(primitive) => Some(primitive),
                _ => None,
            })
    }

    pub fn field(&self, field: impl Into<FieldId>) -> &Field {
        let field_id = field.into();
        assert_eq!(self.id, field_id.model);
        &self.fields[field_id.index]
    }

    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn field_by_name_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.fields.iter_mut().find(|field| field.name == name)
    }

    pub fn find_by_id(&self, schema: &Schema, input: impl stmt::substitute::Input) -> stmt::Query {
        schema.query(self.primary_key.query).apply(input)
    }

    /// Iterate over the fields used for the model's primary key.
    /// TODO: extract type?
    pub fn primary_key_fields(&self) -> impl ExactSizeIterator<Item = &'_ Field> {
        self.primary_key
            .fields
            .iter()
            .map(|pk_field| &self.fields[pk_field.index])
    }

    pub(crate) fn primary_key_primitives(&self) -> impl Iterator<Item = &'_ FieldPrimitive> {
        self.primary_key_fields()
            .map(|field| field.ty.expect_primitive())
    }
}

impl ModelId {
    /// Create a `FieldId` representing the current model's field at index
    /// `index`.
    pub const fn field(self, index: usize) -> FieldId {
        FieldId { model: self, index }
    }

    pub(crate) const fn placeholder() -> ModelId {
        ModelId(usize::MAX)
    }
}

impl From<&ModelId> for ModelId {
    fn from(src: &ModelId) -> ModelId {
        *src
    }
}

impl From<&mut ModelId> for ModelId {
    fn from(src: &mut ModelId) -> ModelId {
        *src
    }
}

impl From<&Model> for ModelId {
    fn from(value: &Model) -> Self {
        value.id
    }
}

impl fmt::Debug for ModelId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ModelId({})", self.0)
    }
}
