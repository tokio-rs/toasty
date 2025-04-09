#[macro_use]
mod fmt;
use fmt::ToSql;

mod cte;

mod delim;
use delim::{Comma, Delimited, Period};

mod flavor;
use flavor::Flavor;

mod ident;
use ident::Ident;

mod params;
pub use params::{Params, Placeholder};

// Fragment serializers
mod column_def;
mod expr;
mod name;
mod stmt;
mod ty;
mod value;

use crate::stmt::Statement;

use toasty_core::schema::db;

/// Serialize a statement to a SQL string
#[derive(Debug)]
pub struct Serializer<'a> {
    /// Schema against which the statement is to be serialized
    schema: &'a db::Schema,

    /// The database flavor handles the differences between SQL dialects and
    /// supported features.
    flavor: Flavor,
}

struct Formatter<'a, T> {
    /// Handle to the serializer
    serializer: &'a Serializer<'a>,

    /// Where to write the serialized SQL
    dst: &'a mut String,

    /// Where to store parameters
    params: &'a mut T,

    /// Current query depth. This is used to determine the nesting level when
    /// generating names
    depth: usize,
}

impl Serializer<'_> {
    pub fn serialize(&self, stmt: &Statement, params: &mut impl Params) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            serializer: self,
            dst: &mut ret,
            params,
            depth: 0,
        };

        stmt.to_sql(&mut fmt);

        ret.push(';');
        ret
    }

    fn table_name(&self, id: impl Into<db::TableId>) -> Ident<&str> {
        let table = self.schema.table(id.into());
        Ident(&table.name)
    }

    fn column_name(&self, id: impl Into<db::ColumnId>) -> Ident<&str> {
        let column = self.schema.column(id.into());
        Ident(&column.name)
    }
}
