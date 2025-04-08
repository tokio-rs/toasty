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
    dst: &'a mut String,
    params: &'a mut T,
    serializer: &'a Serializer<'a>,
}

impl Serializer<'_> {
    pub fn serialize(&self, stmt: &Statement, params: &mut impl Params) -> String {
        println!("SERIALIZING: {stmt:#?}");
        let mut ret = String::new();

        let mut fmt = Formatter {
            dst: &mut ret,
            params,
            serializer: self,
        };

        match stmt {
            Statement::CreateIndex(stmt) => stmt.to_sql(&mut fmt),
            Statement::CreateTable(stmt) => stmt.to_sql(&mut fmt),
            Statement::DropTable(stmt) => stmt.to_sql(&mut fmt),
            Statement::Delete(stmt) => stmt.to_sql(&mut fmt),
            Statement::Insert(stmt) => stmt.to_sql(&mut fmt),
            Statement::Query(stmt) => stmt.to_sql(&mut fmt),
            Statement::Update(stmt) => stmt.to_sql(&mut fmt),
        }

        println!("SERIALIZED: {}", ret);

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
