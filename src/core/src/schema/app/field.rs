mod primitive;
pub use primitive::FieldPrimitive;

use super::*;

use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Field {
    /// Uniquely identifies the field within the containing model.
    pub id: FieldId,

    /// The field name
    pub name: String,

    /// Primitive, relation, composite, ...
    pub ty: FieldTy,

    /// True if the field can be nullable (`None` in Rust).
    pub nullable: bool,

    /// True if the field is part of the primary key
    pub primary_key: bool,

    /// True if toasty is responsible for populating the value of the field
    pub auto: Option<Auto>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct FieldId {
    pub model: ModelId,
    pub index: usize,
}

#[derive(PartialEq)]
pub enum FieldTy {
    Primitive(FieldPrimitive),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}

impl Field {
    pub fn is_relation(&self) -> bool {
        self.ty.is_relation()
    }

    /// Returns a fully qualified name for the field.
    pub fn full_name(&self, schema: &crate::Schema) -> String {
        let model = schema.model(self.id.model);
        format!("{}::{}", model.name.upper_camel_case(), self.name)
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
    pub fn relation_target<'a>(&self, schema: &'a crate::Schema) -> Option<&'a Model> {
        self.relation_target_id().map(|id| schema.model(id))
    }

    /*
    pub fn primitives(&self) -> Box<dyn Iterator<Item = &FieldPrimitive> + '_> {
        match &self.ty {
            FieldTy::Primitive(primitive) => Box::new(Some(primitive).into_iter()),
            FieldTy::BelongsTo(belongs_to) => Box::new(
                belongs_to
                    .foreign_key
                    .fields
                    .iter()
                    .map(|fk_field| &fk_field.primitive),
            ),
            FieldTy::HasMany(_) | FieldTy::HasOne(_) => Box::new(None.into_iter()),
        }
    }
    */

    /*
    pub(crate) fn primitives_mut(&mut self) -> Box<dyn Iterator<Item = &mut FieldPrimitive> + '_> {
        match &mut self.ty {
            FieldTy::Primitive(primitive) => Box::new(Some(primitive).into_iter()),
            FieldTy::BelongsTo(belongs_to) => Box::new(
                belongs_to
                    .foreign_key
                    .fields
                    .iter_mut()
                    .map(|fk_field| &mut fk_field.primitive),
            ),
            FieldTy::HasMany(_) | FieldTy::HasOne(_) => Box::new(None.into_iter()),
        }
    }
    */

    /// The type the field **evaluates** too. This is the "expression type".
    pub fn expr_ty(&self) -> &stmt::Type {
        match &self.ty {
            FieldTy::Primitive(primitive) => &primitive.ty,
            FieldTy::BelongsTo(belongs_to) => &belongs_to.expr_ty,
            FieldTy::HasMany(has_many) => &has_many.expr_ty,
            FieldTy::HasOne(has_one) => &has_one.expr_ty,
        }
    }

    pub fn pair(&self) -> Option<FieldId> {
        match &self.ty {
            FieldTy::Primitive(_) => None,
            FieldTy::BelongsTo(belongs_to) => Some(belongs_to.pair),
            FieldTy::HasMany(has_many) => Some(has_many.pair),
            FieldTy::HasOne(has_one) => Some(has_one.pair),
        }
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

    pub fn is_relation(&self) -> bool {
        matches!(
            self,
            Self::BelongsTo(..) | Self::HasMany(..) | Self::HasOne(..)
        )
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
            Self::BelongsTo(ty) => ty.fmt(fmt),
            Self::HasMany(ty) => ty.fmt(fmt),
            FieldTy::HasOne(ty) => ty.fmt(fmt),
        }
    }
}

impl FieldId {
    pub(crate) fn placeholder() -> FieldId {
        FieldId {
            model: ModelId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl Into<FieldId> for &FieldId {
    fn into(self) -> FieldId {
        *self
    }
}

impl Into<FieldId> for &Field {
    fn into(self) -> FieldId {
        self.id
    }
}

impl Into<usize> for FieldId {
    fn into(self) -> usize {
        self.index
    }
}

impl fmt::Debug for FieldId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "FieldId({}/{})", self.model.0, self.index)
    }
}
