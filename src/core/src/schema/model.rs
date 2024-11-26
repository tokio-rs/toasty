pub(crate) mod attr;
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

    /// Describes how to lower the model to a table
    pub lowering: Lowering,

    /// Fields contained by the model
    pub fields: Vec<Field>,

    /// References the index that represents the model's primary key. This must
    /// be a unique index.
    pub primary_key: PrimaryKey,

    /// Prepared queries that query this model
    pub queries: Vec<QueryId>,

    pub indices: Vec<ModelIndex>,
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

    // pub fn find_by_id(&self, schema: &Schema, input: impl stmt::substitute::Input) -> stmt::Query {
    //     schema.query(self.primary_key.query).apply(input)
    // }

    /*
    pub fn update_stmt(&self, selection: stmt::Query) -> stmt::Update {
        assert_eq!(selection.source(), self.id);

        let expr = stmt::ExprRecord::from_iter(
            std::iter::repeat(stmt::Expr::null()).take(self.fields.len()),
        );

        stmt::Update {
            selection,
            fields: stmt::PathFieldSet::default(),
            expr,
            condition: None,
            returning: false,
        }
    }
    */

    /// Iterate over the fields used for the model's primary key.
    /// TODO: extract type?
    pub fn primary_key_fields<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a Field> + 'a {
        self.primary_key
            .fields
            .iter()
            .map(|pk_field| &self.fields[pk_field.index])
    }

    pub fn primary_key_primitives<'a>(&'a self) -> impl Iterator<Item = &'a FieldPrimitive> + 'a {
        self.primary_key_fields()
            .map(|field| field.ty.expect_primitive())
    }

    pub(crate) fn primary_key_primitives_mut<'a>(
        &'a mut self,
    ) -> impl Iterator<Item = &'a mut FieldPrimitive> + 'a {
        // Some stupidly annoying code to avoid unsafe...
        let mut fields = self.fields.iter_mut().map(Some).collect::<Vec<_>>();
        self.primary_key
            .fields
            .iter()
            .map(move |pk_field| fields[pk_field.index].take().unwrap())
            .map(|field| field.ty.expect_primitive_mut())
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
