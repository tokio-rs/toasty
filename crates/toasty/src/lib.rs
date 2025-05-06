mod batch;
pub use batch::CreateMany;

pub mod cursor;
pub use cursor::Cursor;

pub mod db;
pub use db::Db;

// TODO: move to `db` module
pub mod driver;

mod engine;

mod model;
pub use model::Model;

pub mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

pub mod schema;

pub mod stmt;
pub use stmt::Statement;

pub use toasty_macros::{create, query, Model};

pub use anyhow::{Error, Result};

#[doc(hidden)]
pub mod codegen_support {
    pub use crate::{
        batch::CreateMany,
        cursor::{Cursor, FromCursor},
        relation::Relation,
        relation::{BelongsTo, HasMany, HasOne},
        stmt::{self, Id, IntoExpr, IntoInsert, IntoSelect, Path},
        Db, Error, Model, Result, Statement,
    };
    pub use std::{convert::Into, default::Default, option::Option};
    pub use toasty_core::{
        driver,
        schema::{
            self,
            app::{FieldId, ModelId},
        },
        stmt::{Type, Value, ValueRecord, ValueStream},
    };
}
