//! Mapping between app-level models and database-level tables.
//!
//! The types in this module define how each model field corresponds to one or
//! more database columns. The mapping supports:
//!
//! - Primitive fields that map 1:1 to a column
//! - Embedded structs that flatten into multiple columns
//! - Embedded enums stored as a discriminant column plus per-variant data columns
//! - Relation fields that have no direct column storage
//!
//! The root type is [`Mapping`], which holds a [`Model`] entry for each model.
//! Each `Model` contains per-field [`Field`] mappings and the expression
//! templates ([`Model::model_to_table`] and [`TableToModel`]) used during
//! query lowering.
//!
//! # Examples
//!
//! ```ignore
//! use toasty_core::schema::mapping::Mapping;
//!
//! // Access the mapping for a specific model
//! let model_mapping = mapping.model(model_id);
//! println!("backed by table {:?}", model_mapping.table);
//! ```

mod field;
pub use field::{EnumVariant, Field, FieldEnum, FieldPrimitive, FieldRelation, FieldStruct};

mod item_collection;
pub use item_collection::ItemCollection;

mod model;
pub use model::{Model, TableToModel};

use super::app::ModelId;
use indexmap::IndexMap;

/// Defines the correspondence between app-level models and database-level
/// tables.
///
/// The mapping is constructed during schema building and remains immutable at
/// runtime. It provides the translation layer that enables the query engine to
/// convert model-oriented statements into table-oriented statements during the
/// lowering phase.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::mapping::Mapping;
/// use indexmap::IndexMap;
///
/// let mapping = Mapping { models: IndexMap::new() };
/// assert_eq!(mapping.models.len(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct Mapping {
    /// Per-model mappings indexed by model identifier.
    pub models: IndexMap<ModelId, Model>,
}

impl Mapping {
    /// Returns the mapping for the specified model.
    ///
    /// # Panics
    ///
    /// Panics if the model ID does not exist in the mapping.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let model_mapping = mapping.model(model_id);
    /// println!("table: {:?}", model_mapping.table);
    /// ```
    pub fn model(&self, id: impl Into<ModelId>) -> &Model {
        self.models.get(&id.into()).expect("invalid model ID")
    }

    /// Returns a mutable reference to the mapping for the specified model.
    ///
    /// # Panics
    ///
    /// Panics if the model ID does not exist in the mapping.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let model_mapping = mapping.model_mut(model_id);
    /// // modify fields, columns, etc.
    /// ```
    pub fn model_mut(&mut self, id: impl Into<ModelId>) -> &mut Model {
        self.models.get_mut(&id.into()).expect("invalid model ID")
    }
}
