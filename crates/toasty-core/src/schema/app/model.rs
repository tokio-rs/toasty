use super::{Field, FieldId, Index, Name, PrimaryKey};
use crate::{driver, stmt, Result};
use std::fmt;

#[derive(Debug, Clone)]
pub enum Model {
    /// Root model that maps to a database table and can be queried directly
    Root(ModelRoot),
    /// Embedded struct model that is flattened into its parent model's table
    EmbeddedStruct(EmbeddedStruct),
    /// Embedded enum model stored as a discriminant integer column
    EmbeddedEnum(EmbeddedEnum),
}

#[derive(Debug, Clone)]
pub struct ModelRoot {
    /// Uniquely identifies the model within the schema
    pub id: ModelId,

    /// Name of the model
    pub name: Name,

    /// Fields contained by the model
    pub fields: Vec<Field>,

    /// The primary key for this model. Root models must have a primary key.
    pub primary_key: PrimaryKey,

    /// If the schema specifies a table to map the model to, this is set.
    pub table_name: Option<String>,

    /// Indices defined on this model.
    pub indices: Vec<Index>,
}

impl ModelRoot {
    pub fn find_by_id(&self, mut input: impl stmt::Input) -> stmt::Query {
        let filter = match &self.primary_key.fields[..] {
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
    pub fn primary_key_fields(&self) -> impl ExactSizeIterator<Item = &'_ Field> {
        self.primary_key
            .fields
            .iter()
            .map(|pk_field| &self.fields[pk_field.index])
    }

    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name.app_name == name)
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in &self.fields {
            field.verify(db)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedStruct {
    /// Uniquely identifies the model within the schema
    pub id: ModelId,

    /// Name of the model
    pub name: Name,

    /// Fields contained by the embedded struct
    pub fields: Vec<Field>,
}

impl EmbeddedStruct {
    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        for field in &self.fields {
            field.verify(db)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedEnum {
    /// Uniquely identifies the model within the schema
    pub id: ModelId,

    /// Name of the model
    pub name: Name,

    /// The enum's variants
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// The Rust variant name
    pub name: Name,

    /// The discriminant value stored in the database column
    pub discriminant: i64,
}

impl EmbeddedEnum {
    pub(crate) fn verify(&self, _db: &driver::Capability) -> Result<()> {
        Ok(())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModelId(pub usize);

impl Model {
    pub fn id(&self) -> ModelId {
        match self {
            Model::Root(root) => root.id,
            Model::EmbeddedStruct(embedded) => embedded.id,
            Model::EmbeddedEnum(e) => e.id,
        }
    }

    pub fn name(&self) -> &Name {
        match self {
            Model::Root(root) => &root.name,
            Model::EmbeddedStruct(embedded) => &embedded.name,
            Model::EmbeddedEnum(e) => &e.name,
        }
    }

    /// Returns true if this is a root model (has a table and primary key)
    pub fn is_root(&self) -> bool {
        matches!(self, Model::Root(_))
    }

    /// Returns true if this is an embedded model (flattened into parent)
    pub fn is_embedded(&self) -> bool {
        matches!(self, Model::EmbeddedStruct(_) | Model::EmbeddedEnum(_))
    }

    /// Returns true if this model can be the target of a relation
    pub fn can_be_relation_target(&self) -> bool {
        self.is_root()
    }

    pub fn as_root(&self) -> Option<&ModelRoot> {
        match self {
            Model::Root(root) => Some(root),
            _ => None,
        }
    }

    /// Returns a reference to the root model data, panicking if this is not a root model.
    pub fn expect_root(&self) -> &ModelRoot {
        match self {
            Model::Root(root) => root,
            Model::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
            Model::EmbeddedEnum(_) => panic!("expected root model, found embedded enum"),
        }
    }

    /// Returns a mutable reference to the root model data, panicking if this is not a root model.
    pub fn expect_root_mut(&mut self) -> &mut ModelRoot {
        match self {
            Model::Root(root) => root,
            Model::EmbeddedStruct(_) => panic!("expected root model, found embedded struct"),
            Model::EmbeddedEnum(_) => panic!("expected root model, found embedded enum"),
        }
    }

    /// Returns a reference to the embedded struct data, panicking if this is not an embedded struct.
    pub fn expect_embedded_struct(&self) -> &EmbeddedStruct {
        match self {
            Model::EmbeddedStruct(embedded) => embedded,
            Model::Root(_) => panic!("expected embedded struct, found root model"),
            Model::EmbeddedEnum(_) => panic!("expected embedded struct, found embedded enum"),
        }
    }

    /// Returns a reference to the embedded enum data, panicking if this is not an embedded enum.
    pub fn expect_embedded_enum(&self) -> &EmbeddedEnum {
        match self {
            Model::EmbeddedEnum(e) => e,
            Model::Root(_) => panic!("expected embedded enum, found root model"),
            Model::EmbeddedStruct(_) => panic!("expected embedded enum, found embedded struct"),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        match self {
            Model::Root(root) => root.verify(db),
            Model::EmbeddedStruct(embedded) => embedded.verify(db),
            Model::EmbeddedEnum(e) => e.verify(db),
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
        value.id()
    }
}

impl From<&ModelRoot> for ModelId {
    fn from(value: &ModelRoot) -> Self {
        value.id
    }
}

impl fmt::Debug for ModelId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ModelId({})", self.0)
    }
}
