use super::*;
use toasty_core::schema::{app, Name};
use toasty_core::stmt;

/// Macro-time representation of a BelongsTo relation
///
/// References the target model by ModelId (available via `<T as Model>::ID`),
/// but eliminates FieldId usage to avoid circular dependencies.
#[derive(Debug, Clone)]
pub struct BelongsTo {
    /// ModelId of the target model (e.g., `<User as Model>::ID`)
    pub target: app::ModelId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,

    /// Foreign key field names (will be resolved to FieldIds later)
    pub foreign_key_fields: Vec<String>,
    // Note: No `pair` field - this is resolved during schema registration
    // when all models are known and cross-references can be established
}

/// Macro-time representation of a HasMany relation
#[derive(Debug, Clone)]
pub struct HasMany {
    /// ModelId of the target model (e.g., `<Todo as Model>::ID`)
    pub target: app::ModelId,

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
    /// ModelId of the target model (e.g., `<Profile as Model>::ID`)
    pub target: app::ModelId,

    /// The association's expression type. This is the type the field evaluates
    /// to from a user's point of view.
    pub expr_ty: stmt::Type,
    // Note: No `pair` field - this is resolved during schema registration
    // when all models are known and cross-references can be established
}

impl BelongsTo {
    /// Create a new macro belongs-to relation
    pub fn new(target: app::ModelId, expr_ty: stmt::Type, foreign_key_fields: Vec<String>) -> Self {
        Self {
            target,
            expr_ty,
            foreign_key_fields,
        }
    }
}

impl HasMany {
    /// Create a new macro has-many relation
    pub fn new(target: app::ModelId, expr_ty: stmt::Type, singular: Name) -> Self {
        Self {
            target,
            expr_ty,
            singular,
        }
    }
}

impl HasOne {
    /// Create a new macro has-one relation
    pub fn new(target: app::ModelId, expr_ty: stmt::Type) -> Self {
        Self {
            target,
            expr_ty,
        }
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
