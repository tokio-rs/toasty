use super::*;
use toasty_core::schema::app;

/// Represents a field's schema as known at macro compilation time.
///
/// This is the "incomplete" version of `toasty_core::schema::app::Field` that contains only
/// information available to the macro. Notably missing:
/// - FieldId (depends on ModelId which isn't assigned yet)
/// - Resolved relation pairs (require cross-model analysis)
#[derive(Debug, Clone)]
pub struct Field {
    /// The field name
    pub name: String,

    /// Primitive, relation, composite, ...
    pub ty: FieldTy,

    /// True if the field can be nullable (`None` in Rust).
    pub nullable: bool,

    /// True if the field is part of the primary key
    pub primary_key: bool,

    /// True if toasty is responsible for populating the value of the field
    pub auto: Option<app::Auto>,

    /// Any additional field constraints
    pub constraints: Vec<app::Constraint>,
}

/// Field type with unresolved references
#[derive(Debug, Clone)]
pub enum FieldTy {
    /// Primitive field (no references to resolve)
    Primitive(app::FieldPrimitive),

    /// Belongs-to relation (references target ModelId, not FieldId)
    BelongsTo(BelongsTo),

    /// Has-many relation (references target ModelId, not FieldId)
    HasMany(HasMany),

    /// Has-one relation (references target ModelId, not FieldId)
    HasOne(HasOne),
}

impl Field {
    /// Create a new field
    pub fn new(
        name: String,
        ty: FieldTy,
        nullable: bool,
        primary_key: bool,
        auto: Option<app::Auto>,
        constraints: Vec<app::Constraint>,
    ) -> Self {
        Self {
            name,
            ty,
            nullable,
            primary_key,
            auto,
            constraints,
        }
    }

    /// Check if this field is a relation
    pub fn is_relation(&self) -> bool {
        matches!(
            self.ty,
            FieldTy::BelongsTo(_) | FieldTy::HasMany(_) | FieldTy::HasOne(_)
        )
    }

    /// Get the target TypeId if this is a relation field
    pub fn relation_target(&self) -> Option<std::any::TypeId> {
        match &self.ty {
            FieldTy::BelongsTo(rel) => Some(rel.target),
            FieldTy::HasMany(rel) => Some(rel.target),
            FieldTy::HasOne(rel) => Some(rel.target),
            FieldTy::Primitive(_) => None,
        }
    }
}

impl FieldTy {
    /// Get primitive field type, panicking if not primitive
    pub fn expect_primitive(&self) -> &app::FieldPrimitive {
        match self {
            Self::Primitive(primitive) => primitive,
            _ => panic!("expected primitive field type"),
        }
    }

    /// Get primitive field type mutably, panicking if not primitive
    pub fn expect_primitive_mut(&mut self) -> &mut app::FieldPrimitive {
        match self {
            Self::Primitive(primitive) => primitive,
            _ => panic!("expected primitive field type"),
        }
    }

    /// Get belongs-to relation, panicking if not belongs-to
    pub fn expect_belongs_to(&self) -> &BelongsTo {
        match self {
            Self::BelongsTo(belongs_to) => belongs_to,
            _ => panic!("expected belongs-to field type"),
        }
    }

    /// Get has-many relation, panicking if not has-many
    pub fn expect_has_many(&self) -> &HasMany {
        match self {
            Self::HasMany(has_many) => has_many,
            _ => panic!("expected has-many field type"),
        }
    }

    /// Get has-one relation, panicking if not has-one
    pub fn expect_has_one(&self) -> &HasOne {
        match self {
            Self::HasOne(has_one) => has_one,
            _ => panic!("expected has-one field type"),
        }
    }
}
