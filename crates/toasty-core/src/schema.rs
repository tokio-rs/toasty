//! Schema representation for Toasty, split into three layers.
//!
//! - [`app`] -- Model-level definitions: fields, relations, constraints. This
//!   is what the generated Rust code sees.
//! - [`db`] -- Table/column-level definitions. This is what the database sees.
//! - [`mapping`] -- Connects app fields to database columns, supporting
//!   non-1:1 mappings such as embedded structs and enums.
//!
//! The top-level [`Schema`] struct ties all three layers together and is
//! constructed via [`Builder`].
//!
//! # Examples
//!
//! ```ignore
//! use toasty_core::schema::{Schema, Builder};
//!
//! let schema = Builder::new()
//!     .build(app_schema, &driver_capability)
//!     .expect("schema should be valid");
//!
//! // Look up the database table backing a model
//! let table = schema.table_for(model_id);
//! ```

/// Application-level (model-oriented) schema definitions.
pub mod app;

mod builder;
pub use builder::Builder;

/// Database-level (table/column-oriented) schema definitions.
pub mod db;

/// Mapping between the app layer and the database layer.
pub mod mapping;
use mapping::Mapping;

mod name;
pub use name::Name;

mod verify;

use crate::Result;
use app::ModelId;
use db::{Table, TableId};

/// The combined schema: app-level models, database-level tables, and the
/// mapping that connects them.
///
/// Constructed with [`Builder`] and validated on creation. Immutable at runtime.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::Schema;
///
/// fn inspect(schema: &Schema) {
///     for (id, model) in &schema.app.models {
///         let table = schema.table_for(*id);
///         println!("{} -> {}", model.name().snake_case(), table.name);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Schema {
    /// Application-level schema.
    pub app: app::Schema,

    /// Database-level schema.
    pub db: db::Schema,

    /// Maps the app-level schema to the db-level schema.
    pub mapping: Mapping,
}

impl Schema {
    /// Returns a new [`Builder`] for constructing a `Schema`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toasty_core::Schema;
    ///
    /// let builder = Schema::builder();
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Returns the mapping for the given model.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not correspond to a model in the schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toasty_core::Schema;
    ///
    /// let mapping = schema.mapping_for(model_id);
    /// println!("table: {:?}", mapping.table);
    /// ```
    pub fn mapping_for(&self, id: impl Into<ModelId>) -> &mapping::Model {
        self.mapping.model(id)
    }

    /// Returns the database table that stores the given model.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not correspond to a model in the schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let table = schema.table_for(model_id);
    /// println!("table name: {}", table.name);
    /// ```
    pub fn table_for(&self, id: impl Into<ModelId>) -> &Table {
        self.db.table(self.table_id_for(id))
    }

    /// Returns the [`TableId`] for the table that stores the given model.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not correspond to a model in the schema.
    pub fn table_id_for(&self, id: impl Into<ModelId>) -> TableId {
        self.mapping.model(id).table
    }
}
