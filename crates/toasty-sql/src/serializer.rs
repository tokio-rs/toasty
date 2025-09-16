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
use toasty_core::stmt::TableRef;

/// Serialize a statement to a SQL string
#[derive(Debug)]
pub struct Serializer<'a> {
    /// Schema against which the statement is to be serialized
    schema: &'a db::Schema,

    /// The database flavor handles the differences between SQL dialects and
    /// supported features.
    flavor: Flavor,
}

/// Table context for a specific query level
#[derive(Debug, Clone)]
struct TableContext {
    /// Tables available at this level (from FROM clause)
    tables: Vec<TableRef>,
    /// Whether this level uses table aliases
    has_aliases: bool,
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

    /// Stack of table contexts for each query nesting level
    table_contexts: Vec<TableContext>,
}

impl Serializer<'_> {
    pub fn serialize(&self, stmt: &Statement, params: &mut impl Params) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            serializer: self,
            dst: &mut ret,
            params,
            depth: 0,
            table_contexts: Vec::new(),
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

impl<T: Params> Formatter<'_, T> {
    /// Push a new table context for a query level
    fn push_table_context(&mut self, tables: Vec<TableRef>) {
        let has_aliases = tables.iter().any(|table| table.is_cte());
        self.table_contexts.push(TableContext { tables, has_aliases });
    }

    /// Pop the current table context when exiting a query level
    fn pop_table_context(&mut self) {
        self.table_contexts.pop();
    }

    /// Resolve an ExprColumn and write it directly to the formatter
    fn write_column(&mut self, expr_column: &toasty_core::stmt::ExprColumn) {
        let target_depth = self.depth - expr_column.nesting;

        // Get the table context for the target depth
        if let Some(context) = self.table_contexts.get(target_depth) {
            if let Some(table_ref) = context.tables.get(expr_column.table) {
                match table_ref {
                    TableRef::Table(table_id) => {
                        // For regular tables, get the actual column name
                        let table = self.serializer.schema.table(*table_id);
                        if let Some(column) = table.columns.get(expr_column.column) {
                            if context.has_aliases {
                                // Use table alias if context has aliases
                                self.dst.push_str(&format!("tbl_{}.", target_depth));
                                Ident(&column.name).to_sql(self);
                            } else {
                                // Direct column name for simple queries
                                Ident(&column.name).to_sql(self);
                            }
                        } else {
                            // Fallback for invalid column index
                            self.dst.push_str(&format!("col_{}", expr_column.column));
                        }
                    }
                    TableRef::Cte { .. } => {
                        // For CTEs, always use alias format
                        self.dst.push_str(&format!("tbl_{}.col_{}", target_depth, expr_column.column));
                    }
                }
            } else {
                // Fallback for invalid table index
                self.dst.push_str(&format!("tbl_{}.col_{}", target_depth, expr_column.column));
            }
        } else {
            // Fallback when no context available
            self.dst.push_str(&format!("tbl_{}.col_{}", target_depth, expr_column.column));
        }
    }
}
