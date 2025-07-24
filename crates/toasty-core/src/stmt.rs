mod assignments;
pub use assignments::{Assignment, AssignmentOp, Assignments};

mod association;
pub use association::Association;

mod cte;
pub use cte::Cte;

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

mod expr_func;
pub use expr_func::ExprFunc;

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

mod expr_pattern;
pub use expr_pattern::ExprPattern;

mod expr_project;
pub use expr_project::ExprProject;

mod expr_record;
pub use expr_record::ExprRecord;

mod expr_reference;
pub use expr_reference::ExprReference;

mod expr_set;
pub use expr_set::ExprSet;

mod expr_set_op;
pub use expr_set_op::ExprSetOp;

mod expr_stmt;
pub use expr_stmt::ExprStmt;

mod expr_ty;
pub use expr_ty::ExprTy;

mod func_count;
pub use func_count::FuncCount;

mod id;
pub use id::Id;

mod insert;
pub use insert::Insert;

mod insert_table;
pub use insert_table::InsertTable;

mod insert_target;
pub use insert_target::InsertTarget;

mod join;
pub use join::{Join, JoinOp};

mod limit;
pub use limit::Limit;

mod node;
pub use node::Node;

mod num;

mod offset;
pub use offset::Offset;

mod op_binary;
pub use op_binary::BinaryOp;

mod order_by;
pub use order_by::OrderBy;

mod order_by_expr;
pub use order_by_expr::OrderByExpr;

mod op_set;
pub use op_set::SetOp;

mod path;
pub use path::Path;

mod path_field_set;
pub use path_field_set::PathFieldSet;

mod projection;
pub use projection::Projection;

mod query;
pub use query::{Lock, Query};

mod returning;
pub use returning::Returning;

mod select;
pub use select::Select;

mod source;
pub use source::{Source, SourceModel};

mod sparse_record;
pub use sparse_record::SparseRecord;

pub mod substitute;

mod table_ref;
pub use table_ref::TableRef;

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

mod with;
pub use with::With;

use crate::{
    schema::{
        app::{Field, FieldId, Model, ModelId},
        db::{Column, ColumnId, TableId},
    },
    stmt, Error, Result,
};

use std::fmt;

#[derive(Clone)]
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
            Self::Query(stmt) => stmt.substitute_ref(&mut input),
            _ => todo!("stmt={self:#?}"),
        }
    }

    /// Attempts to return a reference to an inner [`Delete`].
    ///
    /// * If `self` is a [`Statement::Delete`], a reference to the inner [`Delete`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_delete(&self) -> Option<&Delete> {
        match self {
            Self::Delete(delete) => Some(delete),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Delete`].
    ///
    /// * If `self` is a [`Statement::Delete`], inner [`Delete`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_delete(self) -> Option<Delete> {
        match self {
            Self::Delete(delete) => Some(delete),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Delete`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Delete`].
    pub fn unwrap_delete(self) -> Delete {
        match self {
            Self::Delete(delete) => delete,
            v => panic!("expected `Delete`, found {v:#?}"),
        }
    }

    /// Attempts to return a reference to an inner [`Insert`].
    ///
    /// * If `self` is a [`Statement::Insert`], a reference to the inner [`Insert`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_insert(&self) -> Option<&Insert> {
        match self {
            Self::Insert(insert) => Some(insert),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Insert`].
    ///
    /// * If `self` is a [`Statement::Insert`], inner [`Insert`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_insert(self) -> Option<Insert> {
        match self {
            Self::Insert(insert) => Some(insert),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Insert`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Insert`].
    pub fn unwrap_insert(self) -> Insert {
        match self {
            Self::Insert(insert) => insert,
            v => panic!("expected `Insert`, found {v:#?}"),
        }
    }

    /// Attempts to return a reference to an inner [`Query`].
    ///
    /// * If `self` is a [`Statement::Query`], a reference to the inner [`Query`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_query(&self) -> Option<&Query> {
        match self {
            Self::Query(query) => Some(query),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Query`].
    ///
    /// * If `self` is a [`Statement::Query`], inner [`Query`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_query(self) -> Option<Query> {
        match self {
            Self::Query(query) => Some(query),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Query`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Query`].
    pub fn unwrap_query(self) -> Query {
        match self {
            Self::Query(query) => query,
            v => panic!("expected `Query`, found {v:#?}"),
        }
    }

    /// Attempts to return a reference to an inner [`Update`].
    ///
    /// * If `self` is a [`Statement::Update`], a reference to the inner [`Update`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_update(&self) -> Option<&Update> {
        match self {
            Self::Update(update) => Some(update),
            _ => None,
        }
    }

    /// Consumes `self` and attempts to return the inner [`Update`].
    ///
    /// * If `self` is a [`Statement::Update`], inner [`Update`] is returned wrapped in
    ///   [`Some`].
    /// * Else, [`None`] is returned.
    pub fn into_update(self) -> Option<Update> {
        match self {
            Self::Update(update) => Some(update),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Update`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Update`].
    pub fn unwrap_update(self) -> Update {
        match self {
            Self::Update(update) => update,
            v => panic!("expected `Update`, found {v:#?}"),
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
            Self::Delete(v) => v.fmt(f),
            Self::Insert(v) => v.fmt(f),
            Self::Query(v) => v.fmt(f),
            Self::Update(v) => v.fmt(f),
        }
    }
}
