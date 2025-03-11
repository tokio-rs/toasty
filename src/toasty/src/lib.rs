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

pub mod driver {
    pub use toasty_core::driver::*;
}

pub use toasty_macros::{create, model, query};

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
        stmt::{Type, Value, ValueRecord, ValueStream},
    };
}

#[doc(hidden)]
pub mod codegen_support2 {
    pub use crate::{
        batch::CreateMany,
        cursor::{Cursor, FromCursor},
        relation::Relation2 as Relation,
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
