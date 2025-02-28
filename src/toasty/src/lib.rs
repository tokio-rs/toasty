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

pub mod schema;

pub mod stmt;
pub use stmt::Statement;

mod ty;
pub use ty::Type;

pub mod driver {
    pub use toasty_core::driver::*;
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
    pub use std::{convert::Into, default::Default, option::Option};
    pub use toasty_core::{
        driver,
        schema::{self, app::ModelId},
        stmt::{Value, ValueRecord, ValueStream},
    };
}
