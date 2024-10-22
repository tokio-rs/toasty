mod assignment;
pub use assignment::Assignment;

mod binary_op;
pub use binary_op::BinaryOp;

mod column_def;
pub use column_def::ColumnDef;

mod create_index;
pub use create_index::CreateIndex;

mod create_table;
pub use create_table::CreateTable;

mod delete;
pub use delete::Delete;

mod direction;
pub use direction::Direction;

mod expr;
pub use expr::Expr;

mod expr_and;
pub use expr_and::ExprAnd;

mod expr_begins_with;
pub use expr_begins_with::ExprBeginsWith;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_in_list;
pub use expr_in_list::ExprInList;

mod expr_in_subquery;
pub use expr_in_subquery::ExprInSubquery;

mod expr_like;
pub use expr_like::ExprLike;

mod expr_list;
pub use expr_list::ExprList;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_order_by;
pub use expr_order_by::ExprOrderBy;

mod expr_placeholder;
pub use expr_placeholder::ExprPlaceholder;

mod expr_set;
pub use expr_set::ExprSet;

mod expr_tuple;
pub use expr_tuple::ExprTuple;

mod ident;
pub use ident::Ident;

mod insert;
pub use insert::Insert;

mod name;
pub use name::Name;

mod query;
pub use query::Query;

mod select;
pub use select::Select;

mod serialize;

pub mod substitute;

mod table_with_joins;
pub use table_with_joins::TableWithJoins;

mod ty;
pub use ty::Type;

mod update;
pub use update::Update;

mod values;
pub use values::Values;

use crate::{
    schema::{self, Column, ColumnId, TableId},
    stmt::{self, Value},
    Schema,
};

use std::fmt::{self, Write};

#[derive(Debug, Clone)]
pub enum Statement<'stmt> {
    CreateIndex(CreateIndex<'stmt>),
    CreateTable(CreateTable<'stmt>),
    Delete(Delete<'stmt>),
    Insert(Insert<'stmt>),
    Query(Query<'stmt>),
    Update(Update<'stmt>),
}

/// Used to capture parameters when encoding a SQL statement
pub trait Params<'stmt> {
    fn push(&mut self, param: &stmt::Value<'stmt>);
}

impl<'stmt> Statement<'stmt> {
    pub fn query(
        schema: &Schema,
        table: TableId,
        project: stmt::Expr<'stmt>,
        selection: stmt::Expr<'stmt>,
    ) -> Statement<'stmt> {
        Query {
            body: Box::new(ExprSet::Select(Select {
                project: match project {
                    stmt::Expr::Record(items) => items
                        .into_iter()
                        .map(|item| Expr::from_stmt(schema, table, item))
                        .collect(),
                    expr => {
                        vec![Expr::from_stmt(schema, table, expr)]
                    }
                },
                from: vec![TableWithJoins { table, alias: 0 }],
                selection: Some(Expr::from_stmt(schema, table, selection)),
            })),
        }
        .into()
    }

    pub fn delete(
        schema: &Schema,
        table: TableId,
        selection: stmt::Expr<'stmt>,
    ) -> Statement<'stmt> {
        Delete {
            from: vec![TableWithJoins { table, alias: 0 }],
            selection: Some(Expr::from_stmt(schema, table, selection)),
            returning: None,
        }
        .into()
    }

    pub fn into_insert(self) -> Insert<'stmt> {
        match self {
            Statement::Insert(stmt) => stmt,
            _ => todo!(),
        }
    }
}

impl fmt::Display for Statement<'_> {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl<'stmt> Params<'stmt> for Vec<stmt::Value<'stmt>> {
    fn push(&mut self, value: &stmt::Value<'stmt>) {
        self.push(value.clone());
    }
}
