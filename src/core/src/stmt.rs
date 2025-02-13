mod assignments;
pub use assignments::{Assignment, AssignmentOp, Assignments};

mod association;
pub use association::Association;

mod delete;
pub use delete::Delete;

mod direction;
pub use direction::Direction;

mod entry;
pub use entry::Entry;

mod entry_mut;
pub use entry_mut::EntryMut;

mod entry_path;
pub use entry_path::EntryPath;

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

mod expr_cast;
pub use expr_cast::ExprCast;

mod expr_column;
pub use expr_column::ExprColumn;

mod expr_concat;
pub use expr_concat::ExprConcat;

mod expr_concat_str;
pub use expr_concat_str::ExprConcatStr;

mod expr_enum;
pub use expr_enum::ExprEnum;

mod expr_field;
pub use expr_field::ExprField;

mod expr_in_list;
pub use expr_in_list::ExprInList;

mod expr_in_subquery;
pub use expr_in_subquery::ExprInSubquery;

mod expr_is_null;
pub use expr_is_null::ExprIsNull;

mod expr_key;
pub use expr_key::ExprKey;

mod expr_like;
pub use expr_like::ExprLike;

mod expr_list;
pub use expr_list::ExprList;

mod expr_map;
pub use expr_map::ExprMap;

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
pub use insert::Insert;

mod insert_table;
pub use insert_table::InsertTable;

mod insert_target;
pub use insert_target::InsertTarget;

mod node;
pub use node::Node;

mod op_binary;
pub use op_binary::BinaryOp;

mod op_set;
pub use op_set::SetOp;

mod path;
pub use path::Path;

mod path_field_set;
pub use path_field_set::PathFieldSet;

mod projection;
pub use projection::Projection;

mod query;
pub use query::Query;

mod returning;
pub use returning::Returning;

mod select;
pub use select::Select;

mod source;
pub use source::{Source, SourceModel};

mod sparse_record;
pub use sparse_record::SparseRecord;

pub mod sql;

pub mod substitute;

mod table_with_joins;
pub use table_with_joins::TableWithJoins;

mod ty;
pub use ty::Type;

mod ty_enum;
pub use ty_enum::{EnumVariant, TypeEnum};

mod update;
pub use update::{Update, UpdateTarget};

mod value;
pub use value::Value;

mod values;
pub use values::Values;

mod value_enum;
pub use value_enum::ValueEnum;

mod value_record;
pub use value_record::ValueRecord;

pub mod visit_mut;
pub use visit_mut::VisitMut;

mod value_stream;
pub use value_stream::ValueStream;

pub mod visit;
pub use visit::Visit;

use crate::{
    schema::{
        app::{Field, FieldId, Model, ModelId},
        db::{Column, ColumnId, TableId},
    },
    stmt, Error, Result,
};

use std::fmt;

#[derive(Clone, PartialEq)]
pub enum Statement {
    /// Delete one or more existing records
    Delete(Delete),

    /// Create one or more instances of a model
    Insert(Insert),

    /// Query the database
    Query(Query),

    /// Update one or more existing records
    Update(Update),
}

impl Statement {
    pub fn substitute(&mut self, mut input: impl substitute::Input) {
        match self {
            Statement::Query(stmt) => stmt.substitute_ref(&mut input),
            _ => todo!("stmt={self:#?}"),
        }
    }
}

impl Node for Statement {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_mut(self);
    }
}

impl fmt::Debug for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Delete(v) => v.fmt(f),
            Statement::Insert(v) => v.fmt(f),
            Statement::Query(v) => v.fmt(f),
            Statement::Update(v) => v.fmt(f),
        }
    }
}
