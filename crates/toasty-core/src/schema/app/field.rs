mod primitive;
pub use primitive::FieldPrimitive;

use super::{
    AutoStrategy, BelongsTo, Constraint, Embedded, HasMany, HasOne, Model, ModelId, Schema,
};
use crate::{driver, stmt, Result};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Field {
    /// Uniquely identifies the field within the containing model.
    pub id: FieldId,

    /// The field name
    pub name: FieldName,

    /// Primitive, relation, composite, ...
    pub ty: FieldTy,

    /// True if the field can be nullable (`None` in Rust).
    pub nullable: bool,

    /// True if the field is part of the primary key
    pub primary_key: bool,

    /// Specified if and how Toasty should automatically populate this field for new values
    pub auto: Option<AutoStrategy>,

    /// Any additional field constraints
    pub constraints: Vec<Constraint>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldId {
    pub model: ModelId,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct FieldName {
    pub app_name: String,
    pub storage_name: Option<String>,
}

impl FieldName {
    pub fn storage_name(&self) -> &str {
        self.storage_name.as_ref().unwrap_or(&self.app_name)
    }
}

#[derive(Clone)]
pub enum FieldTy {
    Primitive(FieldPrimitive),
    Embedded(Embedded),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}

impl Field {
    /// Gets the id.
    pub fn id(&self) -> FieldId {
        self.id
    }

    /// Gets the name.
    pub fn name(&self) -> &FieldName {
        &self.name
    }

    /// Gets the type.
    pub fn ty(&self) -> &FieldTy {
        &self.ty
    }

    /// Gets whether the field is nullable.
    pub fn nullable(&self) -> bool {
        self.nullable
    }

    /// Gets the primary key.
    pub fn primary_key(&self) -> bool {
        self.primary_key
    }

    /// Gets the [`Auto`].
    pub fn auto(&self) -> Option<&AutoStrategy> {
        self.auto.as_ref()
    }

    pub fn is_auto_increment(&self) -> bool {
        self.auto().map(|auto| auto.is_increment()).unwrap_or(false)
    }

    pub fn is_relation(&self) -> bool {
        self.ty.is_relation()
    }

    /// Returns a fully qualified name for the field.
    pub fn full_name(&self, schema: &Schema) -> String {
        let model = schema.model(self.id.model);
        format!("{}::{}", model.name.upper_camel_case(), self.name.app_name)
    }

    /// If the field is a relation, return the relation's target ModelId.
    pub fn relation_target_id(&self) -> Option<ModelId> {
        match &self.ty {
            FieldTy::BelongsTo(belongs_to) => Some(belongs_to.target),
            FieldTy::HasMany(has_many) => Some(has_many.target),
            _ => None,
        }
    }

    /// If the field is a relation, return the target of the relation.
    pub fn relation_target<'a>(&self, schema: &'a Schema) -> Option<&'a Model> {
        self.relation_target_id().map(|id| schema.model(id))
    }

    /// The type the field **evaluates** too. This is the "expression type".
    pub fn expr_ty(&self) -> &stmt::Type {
        match &self.ty {
            FieldTy::Primitive(primitive) => &primitive.ty,
            FieldTy::Embedded(embedded) => &embedded.expr_ty,
            FieldTy::BelongsTo(belongs_to) => &belongs_to.expr_ty,
            FieldTy::HasMany(has_many) => &has_many.expr_ty,
            FieldTy::HasOne(has_one) => &has_one.expr_ty,
        }
    }

    pub fn pair(&self) -> Option<FieldId> {
        match &self.ty {
            FieldTy::Primitive(_) => None,
            FieldTy::Embedded(_) => None,
            FieldTy::BelongsTo(belongs_to) => belongs_to.pair,
            FieldTy::HasMany(has_many) => Some(has_many.pair),
            FieldTy::HasOne(has_one) => Some(has_one.pair),
        }
    }

    pub(crate) fn verify(&self, db: &driver::Capability) -> Result<()> {
        if let FieldTy::Primitive(primitive) = &self.ty {
            if let Some(storage_ty) = &primitive.storage_ty {
                storage_ty.verify(db)?;
            }
        }

        Ok(())
    }
}

impl FieldTy {
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Primitive(..))
    }

    pub fn as_primitive(&self) -> Option<&FieldPrimitive> {
        match self {
            Self::Primitive(primitive) => Some(primitive),
            _ => None,
        }
    }

    #[track_caller]
    pub fn expect_primitive(&self) -> &FieldPrimitive {
        match self {
            Self::Primitive(simple) => simple,
            _ => panic!("expected simple field, but was {self:?}"),
        }
    }

    #[track_caller]
    pub fn expect_primitive_mut(&mut self) -> &mut FieldPrimitive {
        match self {
            Self::Primitive(simple) => simple,
            _ => panic!("expected simple field, but was {self:?}"),
        }
    }

    pub fn is_embedded(&self) -> bool {
        matches!(self, Self::Embedded(..))
    }

    pub fn as_embedded(&self) -> Option<&Embedded> {
        match self {
            Self::Embedded(embedded) => Some(embedded),
            _ => None,
        }
    }

    #[track_caller]
    pub fn expect_embedded(&self) -> &Embedded {
        match self {
            Self::Embedded(embedded) => embedded,
            _ => panic!("expected embedded field, but was {self:?}"),
        }
    }

    #[track_caller]
    pub fn expect_embedded_mut(&mut self) -> &mut Embedded {
        match self {
            Self::Embedded(embedded) => embedded,
            _ => panic!("expected embedded field, but was {self:?}"),
        }
    }

    pub fn is_relation(&self) -> bool {
        matches!(
            self,
            Self::BelongsTo(..) | Self::HasMany(..) | Self::HasOne(..)
        )
    }

    pub fn is_has_n(&self) -> bool {
        matches!(self, Self::HasMany(..) | Self::HasOne(..))
    }

    pub fn is_has_many(&self) -> bool {
        matches!(self, Self::HasMany(..))
    }

    pub fn as_has_many(&self) -> Option<&HasMany> {
        match self {
            Self::HasMany(has_many) => Some(has_many),
            _ => None,
        }
    }

    #[track_caller]
    pub fn expect_has_many(&self) -> &HasMany {
        match self {
            Self::HasMany(has_many) => has_many,
            _ => panic!("expected field to be `HasMany`, but was {self:?}"),
        }
    }

    #[track_caller]
    pub fn expect_has_many_mut(&mut self) -> &mut HasMany {
        match self {
            Self::HasMany(has_many) => has_many,
            _ => panic!("expected field to be `HasMany`, but was {self:?}"),
        }
    }

    pub fn as_has_one(&self) -> Option<&HasOne> {
        match self {
            Self::HasOne(has_one) => Some(has_one),
            _ => None,
        }
    }

    pub fn is_has_one(&self) -> bool {
        matches!(self, Self::HasOne(..))
    }

    #[track_caller]
    pub fn expect_has_one(&self) -> &HasOne {
        match self {
            Self::HasOne(has_one) => has_one,
            _ => panic!("expected field to be `HasOne`, but it was {self:?}"),
        }
    }

    #[track_caller]
    pub fn expect_has_one_mut(&mut self) -> &mut HasOne {
        match self {
            Self::HasOne(has_one) => has_one,
            _ => panic!("expected field to be `HasOne`, but it was {self:?}"),
        }
    }

    pub fn is_belongs_to(&self) -> bool {
        matches!(self, Self::BelongsTo(..))
    }

    pub fn as_belongs_to(&self) -> Option<&BelongsTo> {
        match self {
            Self::BelongsTo(belongs_to) => Some(belongs_to),
            _ => None,
        }
    }

    #[track_caller]
    pub fn expect_belongs_to(&self) -> &BelongsTo {
        match self {
            Self::BelongsTo(belongs_to) => belongs_to,
            _ => panic!("expected field to be `BelongsTo`, but was {self:?}"),
        }
    }

    #[track_caller]
    pub fn expect_belongs_to_mut(&mut self) -> &mut BelongsTo {
        match self {
            Self::BelongsTo(belongs_to) => belongs_to,
            _ => panic!("expected field to be `BelongsTo`, but was {self:?}"),
        }
    }
}

impl fmt::Debug for FieldTy {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(ty) => ty.fmt(fmt),
            Self::Embedded(ty) => ty.fmt(fmt),
            Self::BelongsTo(ty) => ty.fmt(fmt),
            Self::HasMany(ty) => ty.fmt(fmt),
            Self::HasOne(ty) => ty.fmt(fmt),
        }
    }
}

impl FieldId {
    pub(crate) fn placeholder() -> Self {
        Self {
            model: ModelId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl From<&Self> for FieldId {
    fn from(val: &Self) -> Self {
        *val
    }
}

impl From<&Field> for FieldId {
    fn from(val: &Field) -> Self {
        val.id
    }
}

impl From<FieldId> for usize {
    fn from(val: FieldId) -> Self {
        val.index
    }
}

impl fmt::Debug for FieldId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "FieldId({}/{})", self.model.0, self.index)
    }
}
