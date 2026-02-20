mod apply_update;
pub use apply_update::{ApplyUpdate, Query};

mod batch;
pub use batch::CreateMany;

pub mod cursor;
pub use cursor::Cursor;

pub mod db;
pub use db::Db;

mod engine;

mod model;
pub use model::{Embed, Model, Register};

mod page;
pub use page::Page;

pub mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

pub mod schema;

pub mod stmt;
pub use stmt::Statement;

pub use toasty_macros::{create, query, Embed, Model};

pub use toasty_core::{Error, Result};

#[doc(hidden)]
pub mod codegen_support {
    pub use crate::{
        apply_update::{ApplyUpdate, Query},
        batch::CreateMany,
        cursor::{Cursor, FromCursor},
        model::generate_unique_id,
        relation::Relation,
        relation::{BelongsTo, HasMany, HasOne},
        stmt::{self, IntoExpr, IntoInsert, IntoSelect, Path},
        Db, Embed, Error, Model, Register, Result, Statement,
    };
    pub use std::{convert::Into, default::Default, option::Option};
    pub use toasty_core as core;
    pub use toasty_core::{
        driver,
        schema::{
            self,
            app::{FieldId, ModelId},
        },
        stmt::{Type, Value, ValueRecord, ValueStream},
    };
}

pub mod driver {
    pub use toasty_core::driver::*;
}
