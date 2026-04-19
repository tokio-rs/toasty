//! Statement AST types for Toasty's query compilation pipeline.
//!
//! This module defines the abstract syntax tree (AST) for statements that
//! Toasty's query engine processes. The top-level type is [`Statement`], which
//! represents one of four operations: [`Query`], [`Insert`], [`Update`], or
//! [`Delete`].
//!
//! Statements exist at two layers:
//!
//! - **Model-level**: references models, fields, and associations from the app
//!   schema. This is what user-facing code produces.
//! - **Table-level**: references tables, columns, and joins from the DB schema.
//!   This is what the query engine lowers model-level statements into before
//!   handing them to a database driver.
//!
//! The query engine pipeline transforms statements through several phases:
//! simplify, lower, plan, and execute. Types in this module appear throughout
//! all phases.
//!
//! # Examples
//!
//! ```ignore
//! use toasty_core::stmt::{Statement, Query, Values};
//!
//! // Create a simple values-based query statement
//! let query = Query::unit();
//! let stmt = Statement::Query(query);
//! assert!(stmt.is_query());
//! ```

mod assignments;
pub use assignments::{Assignment, Assignments};

mod association;
pub use association::Association;

mod condition;
pub use condition::Condition;

mod cte;
pub use cte::Cte;

mod cx;
pub use cx::{DerivedRef, ExprContext, ExprTarget, IntoExprTarget, Resolve, ResolvedRef};

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

mod eval;

mod expr;
pub use expr::Expr;

mod expr_and;
pub use expr_and::ExprAnd;

mod expr_any;
pub use expr_any::ExprAny;

mod expr_arg;
pub use expr_arg::ExprArg;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_cast;
pub use expr_cast::ExprCast;

mod expr_error;
pub use expr_error::ExprError;

mod expr_exists;
pub use expr_exists::ExprExists;

mod expr_func;
pub use expr_func::ExprFunc;

mod expr_in_list;
pub use expr_in_list::ExprInList;

mod expr_in_subquery;
pub use expr_in_subquery::ExprInSubquery;

mod expr_is_null;
pub use expr_is_null::ExprIsNull;

mod expr_is_variant;
pub use expr_is_variant::ExprIsVariant;

mod expr_let;
pub use expr_let::ExprLet;

mod expr_list;
pub use expr_list::ExprList;

mod expr_map;
pub use expr_map::ExprMap;

mod expr_match;
pub use expr_match::{ExprMatch, MatchArm};

mod expr_not;
pub use expr_not::ExprNot;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_project;
pub use expr_project::ExprProject;

mod expr_record;
pub use expr_record::ExprRecord;

mod expr_reference;
pub use expr_reference::{ExprColumn, ExprReference};

mod expr_set;
pub use expr_set::ExprSet;

mod expr_set_op;
pub use expr_set_op::ExprSetOp;

mod expr_stmt;
pub use expr_stmt::ExprStmt;

mod filter;
pub use filter::Filter;

mod hash_index;
pub use hash_index::HashIndex;

mod sorted_index;
pub use sorted_index::SortedIndex;

mod func_count;
pub use func_count::FuncCount;

mod func_last_insert_id;
pub use func_last_insert_id::FuncLastInsertId;

mod insert;
pub use insert::Insert;

mod insert_table;
pub use insert_table::InsertTable;

mod insert_target;
pub use insert_target::InsertTarget;

mod input;
pub use input::{ConstInput, Input, TypedInput};

mod join;
pub use join::{Join, JoinOp};

mod limit;
pub use limit::{Limit, LimitCursor, LimitOffset};

#[cfg(feature = "assert-struct")]
mod like;

mod node;
pub use node::Node;

mod num;

mod op_binary;
pub use op_binary::BinaryOp;

mod order_by;
pub use order_by::OrderBy;

mod order_by_expr;
pub use order_by_expr::OrderByExpr;

mod latest_by;
pub use latest_by::LatestBy;

mod op_set;
pub use op_set::SetOp;

mod path;
pub use path::{Path, PathRoot};

mod path_field_set;
pub use path_field_set::PathFieldSet;

mod projection;
pub use projection::{Project, Projection};

mod query;
pub use query::{Lock, Query};

mod returning;
pub use returning::Returning;

mod select;
pub use select::Select;

mod source;
pub use source::{Source, SourceModel};

mod source_table;
pub use source_table::SourceTable;

mod source_table_id;
pub use source_table_id::SourceTableId;

mod sparse_record;
pub use sparse_record::SparseRecord;

mod substitute;
use substitute::Substitute;

mod table_derived;
pub use table_derived::TableDerived;

mod table_ref;
pub use table_ref::TableRef;

mod table_factor;
pub use table_factor::TableFactor;

mod table_with_joins;
pub use table_with_joins::TableWithJoins;

mod ty;
pub use ty::Type;

mod ty_union;
pub use ty_union::TypeUnion;

#[cfg(feature = "jiff")]
mod ty_jiff;

mod update;
pub use update::{Update, UpdateTarget};

mod value;
pub use value::Value;

mod value_cmp;

mod values;
pub use values::Values;

#[cfg(feature = "jiff")]
mod value_jiff;

mod value_record;
pub use value_record::ValueRecord;

mod value_set;
pub use value_set::ValueSet;

/// Mutable AST visitor trait and helpers.
pub mod visit_mut;
pub use visit_mut::VisitMut;

mod value_list;

mod value_stream;
pub use value_stream::ValueStream;

/// Read-only AST visitor trait and helpers.
pub mod visit;
pub use visit::Visit;

mod with;
pub use with::With;

use crate::schema::db::TableId;
use std::fmt;

/// A top-level statement in Toasty's AST.
///
/// Each variant corresponds to one of the four fundamental database operations.
/// A `Statement` is the primary input to the query engine's compilation
/// pipeline and the output of code generated by `#[derive(Model)]`.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Statement, Query, Values};
///
/// let query = Query::unit();
/// let stmt = Statement::from(query);
/// assert!(stmt.is_query());
/// assert!(!stmt.is_insert());
/// ```
#[derive(Clone, PartialEq)]
pub enum Statement {
    /// Delete one or more existing records.
    Delete(Delete),

    /// Create one or more new records.
    Insert(Insert),

    /// Query (read) records from the database.
    Query(Query),

    /// Update one or more existing records.
    Update(Update),
}

impl Statement {
    /// Returns the statement variant name for logging.
    pub fn name(&self) -> &str {
        match self {
            Statement::Query(_) => "query",
            Statement::Insert(_) => "insert",
            Statement::Update(_) => "update",
            Statement::Delete(_) => "delete",
        }
    }

    /// Substitutes argument placeholders in this statement with concrete values
    /// from `input`.
    pub fn substitute(&mut self, input: impl Input) {
        Substitute::new(input).visit_stmt_mut(self);
    }

    /// Returns `true` if this statement is a query whose body contains only
    /// constant values (no table references or subqueries) and has no CTEs.
    pub fn is_const(&self) -> bool {
        match self {
            Statement::Query(query) => {
                if query.with.is_some() {
                    return false;
                }

                query.body.is_const()
            }
            _ => false,
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

    /// Returns `true` if this statement expects at most one result row.
    pub fn is_single(&self) -> bool {
        match self {
            Statement::Query(q) => q.single,
            Statement::Insert(i) => i.source.single,
            Statement::Update(i) => match &i.target {
                UpdateTarget::Query(q) => q.single,
                UpdateTarget::Model(_) => true,
                _ => false,
            },
            Statement::Delete(d) => d.selection().single,
        }
    }

    /// Consumes `self` and returns the inner [`Update`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Update`].
    pub fn into_update_unwrap(self) -> Update {
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
