use super::{Field, FieldId, FieldPrimitive, FieldTy, Index, Name, PrimaryKey};
use crate::{driver, stmt, Result};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Model {
    /// Uniquely identifies the model within the schema
    pub id: ModelId,

    /// Name of the model
    pub name: Name,

    /// Distinguishes root models (with tables and primary keys) from embedded models
    pub kind: ModelKind,

    pub indices: Vec<Index>,
}

#[derive(Debug, Clone)]
pub enum ModelKind {
    /// Root model that maps to a database table and can be queried directly
    Root(ModelRoot),
    /// Embedded struct model that is flattened into its parent model's table
    EmbeddedStruct(EmbeddedStruct),
}

#[derive(Debug, Clone)]
pub struct ModelRoot {
    /// Fields contained by the model
    pub fields: Vec<Field>,

    /// The primary key for this model. Root models must have a primary key.
    pub primary_key: PrimaryKey,

    /// If the schema specifies a table to map the model to, this is set.
    pub table_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedStruct {
    /// Fields contained by the embedded struct
    pub fields: Vec<Field>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModelId(pub usize);

impl Model {
    /// Returns true if this is a root model (has a table and primary key)
    pub fn is_root(&self) -> bool {
        matches!(self.kind, ModelKind::Root(_))
    }

    /// Returns true if this is an embedded model (flattened into parent)
    pub fn is_embedded(&self) -> bool {
        matches!(self.kind, ModelKind::EmbeddedStruct(_))
    }

    /// Returns the primary key if this is a root model, None if embedded
    pub fn primary_key(&self) -> Option<&PrimaryKey> {
        match &self.kind {
            ModelKind::Root(root) => Some(&root.primary_key),
            ModelKind::EmbeddedStruct(_) => None,
        }
    }

    /// Returns true if this model can be the target of a relation
    pub fn can_be_relation_target(&self) -> bool {
        self.is_root()
    }

    pub fn primitives_mut(&mut self) -> impl Iterator<Item = &mut FieldPrimitive> + '_ {
        self.kind
            .fields_mut()
            .iter_mut()
            .flat_map(|field| match &mut field.ty {
                FieldTy::Primitive(primitive) => Some(primitive),
                _ => None,
            })
    }

    pub fn field(&self, field: impl Into<FieldId>) -> &Field {
        let field_id = field.into();
        assert_eq!(self.id, field_id.model);
        &self.kind.fields()[field_id.index]
    }

    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.kind
            .fields()
            .iter()
            .find(|field| field.name.app_name == name)
    }

    pub fn field_by_name_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.kind
            .fields_mut()
            .iter_mut()
            .find(|field| field.name.app_name == name)
    }

    pub fn find_by_id(&self, mut input: impl stmt::Input) -> stmt::Query {
        let primary_key = self
            .primary_key()
            .expect("find_by_id requires a root model with primary key");

        let filter = match &primary_key.fields[..] {
            [pk_field] => stmt::Expr::eq(
                stmt::Expr::ref_self_field(pk_field),
                input
                    .resolve_arg(&0.into(), &stmt::Projection::identity())
                    .unwrap(),
            ),
            pk_fields => stmt::Expr::and_from_vec(
                pk_fields
                    .iter()
                    .enumerate()
                    .map(|(i, pk_field)| {
                        stmt::Expr::eq(
                            stmt::Expr::ref_self_field(pk_field),
                            input
                                .resolve_arg(&i.into(), &stmt::Projection::identity())
                                .unwrap(),
                        )
                    })
                    .collect(),
            ),
        };

        stmt::Query::new_select(self.id, filter)
    }

    /// Iterate over the fields used for the model's primary key.
    /// Returns None if this is an embedded model.
    /// TODO: extract type?
    pub fn primary_key_fields(&self) -> Option<impl ExactSizeIterator<Item = &'_ Field>> {
        match &self.kind {
            ModelKind::Root(root) => {
                let pk = &root.primary_key;
                Some(pk.fields.iter().map(|pk_field| &root.fields[pk_field.index]))
            }
            ModelKind::EmbeddedStruct(_) => None,
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in self.kind.fields() {
            field.verify(db)?;
        }

        Ok(())
    }
}

impl ModelKind {
    /// Returns a reference to the root model data, panicking if this is not a root model.
    pub fn expect_root(&self) -> &ModelRoot {
        match self {
            ModelKind::Root(root) => root,
            ModelKind::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
        }
    }

    /// Returns a mutable reference to the root model data, panicking if this is not a root model.
    pub fn expect_root_mut(&mut self) -> &mut ModelRoot {
        match self {
            ModelKind::Root(root) => root,
            ModelKind::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
        }
    }

    /// Returns a reference to the embedded struct data, panicking if this is not an embedded struct.
    pub fn expect_embedded_struct(&self) -> &EmbeddedStruct {
        match self {
            ModelKind::EmbeddedStruct(embedded) => embedded,
            ModelKind::Root(_) => panic!("expected embedded struct, found root model"),
        }
    }

    /// Returns the fields for any model kind.
    pub fn fields(&self) -> &[Field] {
        match self {
            ModelKind::Root(root) => &root.fields,
            ModelKind::EmbeddedStruct(embedded) => &embedded.fields,
        }
    }

    /// Returns a mutable slice of fields for any model kind.
    pub fn fields_mut(&mut self) -> &mut [Field] {
        match self {
            ModelKind::Root(root) => &mut root.fields,
            ModelKind::EmbeddedStruct(embedded) => &mut embedded.fields,
        }
    }
}

impl ModelId {
    /// Create a `FieldId` representing the current model's field at index
    /// `index`.
    pub const fn field(self, index: usize) -> FieldId {
        FieldId { model: self, index }
    }

    pub(crate) const fn placeholder() -> Self {
        Self(usize::MAX)
    }
}

impl From<&Self> for ModelId {
    fn from(src: &Self) -> Self {
        *src
    }
}

impl From<&mut Self> for ModelId {
    fn from(src: &mut Self) -> Self {
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
