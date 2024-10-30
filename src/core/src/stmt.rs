mod assignments;
pub use assignments::Assignments;

mod delete;
pub use delete::Delete;

mod direction;
pub use direction::Direction;

pub mod eval;

mod expr;
pub use expr::Expr;

mod expr_and;
pub use expr_and::ExprAnd;

mod expr_arg;
pub use expr_arg::ExprArg;

mod expr_begins_with;
pub use expr_begins_with::ExprBeginsWith;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_column;
pub use expr_column::ExprColumn;

mod expr_concat;
pub use expr_concat::ExprConcat;

mod expr_enum;
pub use expr_enum::ExprEnum;

mod expr_field;
pub use expr_field::ExprField;

mod expr_in_list;
pub use expr_in_list::ExprInList;

mod expr_in_subquery;
pub use expr_in_subquery::ExprInSubquery;

mod expr_like;
pub use expr_like::ExprLike;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_order_by;
pub use expr_order_by::ExprOrderBy;

mod expr_pattern;
pub use expr_pattern::ExprPattern;

mod expr_project;
pub use expr_project::ExprProject;

mod expr_record;
pub use expr_record::ExprRecord;

mod expr_set;
pub use expr_set::ExprSet;

mod expr_set_op;
pub use expr_set_op::ExprSetOp;

mod expr_stmt;
pub use expr_stmt::ExprStmt;

mod expr_ty;
pub use expr_ty::ExprTy;

mod id;
pub use id::Id;

mod insert;
pub use insert::{Insert, InsertTable, InsertTarget};

mod link;
pub use link::Link;

pub mod map;
pub use map::Map;

mod node;
pub use node::Node;

mod op_binary;
pub use op_binary::BinaryOp;

mod op_set;
pub use op_set::SetOp;

mod path;
pub use path::Path;

mod path_step;
pub use path_step::PathStep;

mod path_field_set;
pub use path_field_set::PathFieldSet;

mod projection;
pub use projection::Projection;

mod query;
pub use query::Query;

pub mod record;
pub use record::Record;

mod record_cow;
pub use record_cow::RecordCow;

mod record_stream;
pub use record_stream::RecordStream;

pub(crate) mod resolve;

mod returning;
pub use returning::Returning;

mod select;
pub use select::Select;

mod source;
pub use source::{Source, SourceModel};

pub mod sql;

pub mod substitute;

mod table_with_joins;
pub use table_with_joins::TableWithJoins;

mod ty;
pub use ty::Type;

mod ty_enum;
pub use ty_enum::{EnumVariant, TypeEnum};

mod unlink;
pub use unlink::Unlink;

mod update;
pub use update::{Update, UpdateTarget};

mod value;
pub use value::Value;

mod values;
pub use values::Values;

mod value_enum;
pub use value_enum::ValueEnum;

pub mod visit_mut;
pub use visit_mut::VisitMut;

mod value_stream;
pub use value_stream::ValueStream;

pub mod visit;
pub use visit::Visit;

use crate::{
    schema::{Column, ColumnId, Field, FieldId, Model, ModelId, Schema, TableId},
    stmt, Error, Result,
};

use std::fmt;

#[derive(Clone, PartialEq)]
pub enum Statement<'stmt> {
    /// Delete one or more existing records
    Delete(Delete<'stmt>),

    /// Link one or more associations
    Link(Link<'stmt>),

    /// Create one or more instances of a model
    Insert(Insert<'stmt>),

    /// Query the database
    Query(Query<'stmt>),

    /// Unlink one or more associations
    Unlink(Unlink<'stmt>),

    /// Update one or more existing records
    Update(Update<'stmt>),
}

impl<'stmt> Statement<'stmt> {
    pub fn inputs(&self) -> Vec<Path> {
        todo!()
    }

    /*
    pub fn is_select(&self) -> bool {
        use Statement::*;
        matches!(self, Select(_))
    }

    pub fn is_write(&self) -> bool {
        !self.is_select()
    }
    */
}

impl<'stmt> Node<'stmt> for Statement<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_mut(self);
    }
}

impl<'a> fmt::Debug for Statement<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Delete(v) => v.fmt(f),
            Statement::Link(v) => v.fmt(f),
            Statement::Insert(v) => v.fmt(f),
            Statement::Query(v) => v.fmt(f),
            Statement::Unlink(v) => v.fmt(f),
            Statement::Update(v) => v.fmt(f),
        }
    }
}
