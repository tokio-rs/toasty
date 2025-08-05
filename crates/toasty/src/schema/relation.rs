use super::*;
use toasty_core::schema::Name;
use toasty_core::stmt;

/// Macro-time representation of a BelongsTo relation
///
/// References the target model by TypeId (available via `TypeId::of::<T>()`),
/// eliminating ModelId usage to avoid circular dependencies.
#[derive(Debug, Clone)]
pub struct BelongsTo {
    /// TypeId of the target model (e.g., `TypeId::of::<User>()`)
    pub target: std::any::TypeId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// Foreign key field information
    pub foreign_key: Vec<ForeignKeyField>,
    // Note: No `pair` field - this is resolved during schema registration
    // when all models are known and cross-references can be established
}

/// Represents a foreign key field mapping
#[derive(Debug, Clone)]
pub struct ForeignKeyField {
    /// Source field name (in this model)
    pub source: String,
    /// Target field name (in the target model)
    pub target: String,
}

/// Macro-time representation of a HasMany relation
#[derive(Debug, Clone)]
pub struct HasMany {
    /// TypeId of the target model (e.g., `TypeId::of::<Todo>()`)
    pub target: std::any::TypeId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// Singular item name
    pub singular: Name,
    // Note: No `pair` field - this is resolved during schema registration
    // when all models are known and cross-references can be established
}

/// Macro-time representation of a HasOne relation
#[derive(Debug, Clone)]
pub struct HasOne {
    /// TypeId of the target model (e.g., `TypeId::of::<Profile>()`)
    pub target: std::any::TypeId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,
    // Note: No `pair` field - this is resolved during schema registration
    // when all models are known and cross-references can be established
}

impl BelongsTo {
    /// Create a new macro belongs-to relation
    pub fn new(
        target: std::any::TypeId,
        expr_ty: stmt::Type,
        foreign_key: Vec<ForeignKeyField>,
    ) -> Self {
        Self {
            target,
            expr_ty,
            foreign_key,
        }
    }
}

impl HasMany {
    /// Create a new macro has-many relation
    pub fn new(target: std::any::TypeId, expr_ty: stmt::Type, singular: Name) -> Self {
        Self {
            target,
            expr_ty,
            singular,
        }
    }
}

impl HasOne {
    /// Create a new macro has-one relation
    pub fn new(target: std::any::TypeId, expr_ty: stmt::Type) -> Self {
        Self { target, expr_ty }
    }
}

// Conversion implementations for FieldTy
impl From<BelongsTo> for FieldTy {
    fn from(value: BelongsTo) -> Self {
        Self::BelongsTo(value)
    }
}

impl From<HasMany> for FieldTy {
    fn from(value: HasMany) -> Self {
        Self::HasMany(value)
    }
}

impl From<HasOne> for FieldTy {
    fn from(value: HasOne) -> Self {
        Self::HasOne(value)
    }
}
