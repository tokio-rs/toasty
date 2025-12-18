mod assignments;
pub use assignments::{Assignment, AssignmentOp, Assignments};

mod association;
pub use association::Association;

mod condition;
pub use condition::Condition;

mod cte;
pub use cte::Cte;

mod cx;
pub use cx::{ExprContext, ExprTarget, IntoExprTarget, Resolve, ResolvedRef};

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

mod expr_begins_with;
pub use expr_begins_with::ExprBeginsWith;

mod expr_binary_op;
pub use expr_binary_op::ExprBinaryOp;

mod expr_cast;
pub use expr_cast::ExprCast;

mod expr_concat;
pub use expr_concat::ExprConcat;

mod expr_concat_str;
pub use expr_concat_str::ExprConcatStr;

mod expr_enum;
pub use expr_enum::ExprEnum;

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

mod expr_key;
pub use expr_key::ExprKey;

mod expr_like;
pub use expr_like::ExprLike;

mod expr_list;
pub use expr_list::ExprList;

mod expr_map;
pub use expr_map::ExprMap;

mod expr_not;
pub use expr_not::ExprNot;

mod expr_or;
pub use expr_or::ExprOr;

mod expr_pattern;
pub use expr_pattern::ExprPattern;

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

mod expr_ty;
pub use expr_ty::ExprTy;

mod filter;
pub use filter::Filter;

mod func_count;
pub use func_count::FuncCount;

mod func_last_insert_id;
pub use func_last_insert_id::FuncLastInsertId;

mod id;
pub use id::Id;

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
pub use limit::Limit;

#[cfg(feature = "assert-struct")]
mod like;

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

mod ty_enum;
pub use ty_enum::{EnumVariant, TypeEnum};

#[cfg(feature = "jiff")]
mod ty_jiff;

mod update;
pub use update::{Update, UpdateTarget};

mod value;
pub use value::Value;

mod value_cmp;

mod values;
pub use values::Values;

mod value_enum;
pub use value_enum::ValueEnum;

#[cfg(feature = "jiff")]
mod value_jiff;

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

use crate::schema::db::TableId;
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
    pub fn substitute(&mut self, input: impl Input) {
        Substitute::new(input).visit_stmt_mut(self);
    }

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
