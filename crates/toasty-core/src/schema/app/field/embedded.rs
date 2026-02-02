use super::FieldName;
use crate::schema::app::{AutoStrategy, Constraint};
use crate::schema::Name;

#[derive(Debug, Clone)]
pub struct Embedded {
    /// Name of the embedded struct
    pub name: Name,

    /// Fields of the embedded struct
    pub fields: Vec<EmbeddedField>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedField {
    /// Field name (includes app_name and optional storage_name from #[column])
    pub name: FieldName,

    /// The field's type (can be Primitive or Embedded, not relations)
    /// Relations are forbidden and checked during schema validation
    pub ty: super::FieldTy,

    /// Whether this field is nullable
    pub nullable: bool,

    /// Field attributes (auto, unique, index, constraints, etc.)
    /// Note: primary_key is tracked on the parent Field, not here
    pub attrs: FieldAttr,
}

#[derive(Debug, Clone)]
pub struct FieldAttr {
    /// Specifies if and how Toasty should automatically populate this field
    pub auto: Option<AutoStrategy>,

    /// True if the field should be unique
    pub unique: bool,

    /// True if the field should be indexed
    pub index: bool,

    /// Additional field constraints (e.g., length)
    pub constraints: Vec<Constraint>,
}
