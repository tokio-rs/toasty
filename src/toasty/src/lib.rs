mod batch;
pub use batch::CreateMany;

pub mod cursor;
pub use cursor::Cursor;

mod db;
pub use db::Db;

mod engine;

mod model;
pub use model::Model;

pub mod relation;
pub use relation::Relation;

pub mod stmt;
pub use stmt::Statement;

pub mod driver {
    pub use toasty_core::driver::*;
}

pub mod schema {
    pub use toasty_core::schema::*;
}

pub use toasty_macros::{create, query};

pub use anyhow::{Error, Result};

#[doc(hidden)]
pub mod codegen_support {
    pub use crate::{
        batch::CreateMany,
        cursor::{Cursor, FromCursor},
        relation::{BelongsTo, HasMany},
        stmt::{self, Id, IntoExpr, IntoInsert, IntoSelect, Path},
        Db, Error, Model, Relation, Result, Statement,
    };
    pub use toasty_core::{
        driver,
        schema::{self, app::ModelId},
        stmt::{Value, ValueRecord, ValueStream},
    };
}
