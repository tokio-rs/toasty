#![allow(unused_variables)]

use super::{
    Assignment, Assignments, Association, Condition, Cte, Delete, Expr, ExprAnd, ExprAny, ExprArg,
    ExprBeginsWith, ExprBinaryOp, ExprCast, ExprColumn, ExprError, ExprExists, ExprFunc,
    ExprInList, ExprInSubquery, ExprIsNull, ExprIsVariant, ExprLet, ExprList, ExprMap, ExprMatch,
    ExprNot, ExprOr, ExprProject, ExprRecord, ExprReference, ExprSet, ExprSetOp, ExprStmt, Filter,
    FuncCount, FuncLastInsertId, Insert, InsertTarget, Join, JoinOp, Limit, LimitCursor,
    LimitOffset, Node, OrderBy, OrderByExpr, Path, Projection, Query, Returning, Select, Source,
    SourceModel, SourceTable, SourceTableId, Statement, TableDerived, TableFactor, TableRef,
    TableWithJoins, Type, Update, UpdateTarget, Value, ValueRecord, Values, With,
};

/// Mutable visitor trait for the statement AST.
///
/// Implement this trait to walk and modify the AST in place. Each
/// `visit_*_mut` method has a default implementation that recurses into
/// child nodes via the corresponding free function (e.g.,
/// [`visit_expr_mut`]). Override specific methods to transform nodes of
/// interest.
///
/// Companion helpers:
/// - [`for_each_expr_mut`] -- visits every expression node in post-order.
/// - [`walk_expr_scoped_mut`] -- walks expressions while tracking
///   `Let`/`Map` scope depth.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{visit_mut, Expr, Value, Node};
///
/// let mut expr = Expr::from(Value::from(42_i64));
/// visit_mut::for_each_expr_mut(&mut expr, |e| {
///     // transform expressions in place
/// });
/// ```
pub trait VisitMut {
    /// Dispatches to the appropriate `visit_*_mut` method via [`Node::visit_mut`].
    fn visit_mut<N: Node>(&mut self, i: &mut N)
    where
        Self: Sized,
    {
        i.visit_mut(self);
    }

    /// Visits an [`Assignment`] node mutably.
    ///
    /// The default implementation delegates to [`visit_assignment_mut`].
    fn visit_assignment_mut(&mut self, i: &mut Assignment) {
        visit_assignment_mut(self, i);
    }

    /// Visits an [`Assignments`] node mutably.
    ///
    /// The default implementation delegates to [`visit_assignments_mut`].
    fn visit_assignments_mut(&mut self, i: &mut Assignments) {
        visit_assignments_mut(self, i);
    }

    /// Visits an [`Association`] node mutably.
    ///
    /// The default implementation delegates to [`visit_association_mut`].
    fn visit_association_mut(&mut self, i: &mut Association) {
        visit_association_mut(self, i);
    }

    /// Visits a [`Cte`] (common table expression) node mutably.
    ///
    /// The default implementation delegates to [`visit_cte_mut`].
    fn visit_cte_mut(&mut self, i: &mut Cte) {
        visit_cte_mut(self, i);
    }

    /// Visits an [`Expr`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_mut`].
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        visit_expr_mut(self, i);
    }

    /// Visits an [`ExprAnd`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_and_mut`].
    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd) {
        visit_expr_and_mut(self, i);
    }

    /// Visits an [`ExprAny`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_any_mut`].
    fn visit_expr_any_mut(&mut self, i: &mut ExprAny) {
        visit_expr_any_mut(self, i);
    }

    /// Visits an [`ExprArg`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_arg_mut`].
    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        visit_expr_arg_mut(self, i);
    }

    /// Visits an [`ExprBeginsWith`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_begins_with_mut`].
    fn visit_expr_begins_with_mut(&mut self, i: &mut ExprBeginsWith) {
        visit_expr_begins_with_mut(self, i);
    }

    /// Visits an [`ExprBinaryOp`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_binary_op_mut`].
    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp) {
        visit_expr_binary_op_mut(self, i);
    }

    /// Visits an [`ExprCast`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_cast_mut`].
    fn visit_expr_cast_mut(&mut self, i: &mut ExprCast) {
        visit_expr_cast_mut(self, i);
    }

    /// Visits an [`ExprColumn`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_column_mut`].
    fn visit_expr_column_mut(&mut self, i: &mut ExprColumn) {
        visit_expr_column_mut(self, i);
    }

    /// Visits a default expression node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_default_mut`].
    fn visit_expr_default_mut(&mut self) {
        visit_expr_default_mut(self);
    }

    /// Visits an [`ExprError`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_error_mut`].
    fn visit_expr_error_mut(&mut self, i: &mut ExprError) {
        visit_expr_error_mut(self, i);
    }

    /// Visits an [`ExprExists`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_exists_mut`].
    fn visit_expr_exists_mut(&mut self, i: &mut ExprExists) {
        visit_expr_exists_mut(self, i);
    }

    /// Visits an [`ExprFunc`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_func_mut`].
    fn visit_expr_func_mut(&mut self, i: &mut ExprFunc) {
        visit_expr_func_mut(self, i);
    }

    /// Visits a [`FuncCount`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_func_count_mut`].
    fn visit_expr_func_count_mut(&mut self, i: &mut FuncCount) {
        visit_expr_func_count_mut(self, i);
    }

    /// Visits a [`FuncLastInsertId`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_func_last_insert_id_mut`].
    fn visit_expr_func_last_insert_id_mut(&mut self, i: &mut FuncLastInsertId) {
        visit_expr_func_last_insert_id_mut(self, i);
    }

    /// Visits an [`ExprInList`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_in_list_mut`].
    fn visit_expr_in_list_mut(&mut self, i: &mut ExprInList) {
        visit_expr_in_list_mut(self, i);
    }

    /// Visits an [`ExprInSubquery`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_in_subquery_mut`].
    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery) {
        visit_expr_in_subquery_mut(self, i);
    }

    /// Visits an [`ExprIsNull`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_is_null_mut`].
    fn visit_expr_is_null_mut(&mut self, i: &mut ExprIsNull) {
        visit_expr_is_null_mut(self, i);
    }

    /// Visits an [`ExprIsVariant`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_is_variant_mut`].
    fn visit_expr_is_variant_mut(&mut self, i: &mut ExprIsVariant) {
        visit_expr_is_variant_mut(self, i);
    }

    /// Visits an [`ExprLet`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_let_mut`].
    fn visit_expr_let_mut(&mut self, i: &mut ExprLet) {
        visit_expr_let_mut(self, i);
    }

    /// Visits an [`ExprMap`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_map_mut`].
    fn visit_expr_map_mut(&mut self, i: &mut ExprMap) {
        visit_expr_map_mut(self, i);
    }

    /// Visits an [`ExprMatch`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_match_mut`].
    fn visit_expr_match_mut(&mut self, i: &mut ExprMatch) {
        visit_expr_match_mut(self, i);
    }

    /// Visits an [`ExprNot`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_not_mut`].
    fn visit_expr_not_mut(&mut self, i: &mut ExprNot) {
        visit_expr_not_mut(self, i);
    }

    /// Visits an [`ExprOr`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_or_mut`].
    fn visit_expr_or_mut(&mut self, i: &mut ExprOr) {
        visit_expr_or_mut(self, i);
    }

    /// Visits an [`ExprList`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_list_mut`].
    fn visit_expr_list_mut(&mut self, i: &mut ExprList) {
        visit_expr_list_mut(self, i);
    }

    /// Visits an [`ExprRecord`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_record_mut`].
    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord) {
        visit_expr_record_mut(self, i);
    }

    /// Visits an [`ExprReference`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_reference_mut`].
    fn visit_expr_reference_mut(&mut self, i: &mut ExprReference) {
        visit_expr_reference_mut(self, i);
    }

    /// Visits an [`ExprSet`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_set_mut`].
    fn visit_expr_set_mut(&mut self, i: &mut ExprSet) {
        visit_expr_set_mut(self, i);
    }

    /// Visits an [`ExprSetOp`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_set_op_mut`].
    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp) {
        visit_expr_set_op_mut(self, i);
    }

    /// Visits an [`ExprStmt`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_stmt_mut`].
    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt) {
        visit_expr_stmt_mut(self, i);
    }

    /// Visits a [`Filter`] node mutably.
    ///
    /// The default implementation delegates to [`visit_filter_mut`].
    fn visit_filter_mut(&mut self, i: &mut Filter) {
        visit_filter_mut(self, i);
    }

    /// Visits a [`Condition`] node mutably.
    ///
    /// The default implementation delegates to [`visit_condition_mut`].
    fn visit_condition_mut(&mut self, i: &mut Condition) {
        visit_condition_mut(self, i);
    }

    /// Visits an [`ExprProject`] node mutably.
    ///
    /// The default implementation delegates to [`visit_expr_project_mut`].
    fn visit_expr_project_mut(&mut self, i: &mut ExprProject) {
        visit_expr_project_mut(self, i);
    }

    /// Visits an [`InsertTarget`] node mutably.
    ///
    /// The default implementation delegates to [`visit_insert_target_mut`].
    fn visit_insert_target_mut(&mut self, i: &mut InsertTarget) {
        visit_insert_target_mut(self, i);
    }

    /// Visits a [`Join`] node mutably.
    ///
    /// The default implementation delegates to [`visit_join_mut`].
    fn visit_join_mut(&mut self, i: &mut Join) {
        visit_join_mut(self, i);
    }

    /// Visits a [`Limit`] node mutably.
    ///
    /// The default implementation delegates to [`visit_limit_mut`].
    fn visit_limit_mut(&mut self, i: &mut Limit) {
        visit_limit_mut(self, i);
    }

    /// Visits a [`LimitCursor`] node mutably.
    ///
    /// The default implementation delegates to [`visit_limit_cursor_mut`].
    fn visit_limit_cursor_mut(&mut self, i: &mut LimitCursor) {
        visit_limit_cursor_mut(self, i);
    }

    /// Visits a [`LimitOffset`] node mutably.
    ///
    /// The default implementation delegates to [`visit_limit_offset_mut`].
    fn visit_limit_offset_mut(&mut self, i: &mut LimitOffset) {
        visit_limit_offset_mut(self, i);
    }

    /// Visits an [`OrderBy`] node mutably.
    ///
    /// The default implementation delegates to [`visit_order_by_mut`].
    fn visit_order_by_mut(&mut self, i: &mut OrderBy) {
        visit_order_by_mut(self, i);
    }

    /// Visits an [`OrderByExpr`] node mutably.
    ///
    /// The default implementation delegates to [`visit_order_by_expr_mut`].
    fn visit_order_by_expr_mut(&mut self, i: &mut OrderByExpr) {
        visit_order_by_expr_mut(self, i);
    }

    /// Visits a [`Path`] node mutably.
    ///
    /// The default implementation delegates to [`visit_path_mut`].
    fn visit_path_mut(&mut self, i: &mut Path) {
        visit_path_mut(self, i);
    }

    /// Visits a [`Projection`] node mutably.
    ///
    /// The default implementation delegates to [`visit_projection_mut`].
    fn visit_projection_mut(&mut self, i: &mut Projection) {
        visit_projection_mut(self, i);
    }

    /// Visits a [`Returning`] node mutably.
    ///
    /// The default implementation delegates to [`visit_returning_mut`].
    fn visit_returning_mut(&mut self, i: &mut Returning) {
        visit_returning_mut(self, i);
    }

    /// Visits a [`Source`] node mutably.
    ///
    /// The default implementation delegates to [`visit_source_mut`].
    fn visit_source_mut(&mut self, i: &mut Source) {
        visit_source_mut(self, i);
    }

    /// Visits a [`SourceModel`] node mutably.
    ///
    /// The default implementation delegates to [`visit_source_model_mut`].
    fn visit_source_model_mut(&mut self, i: &mut SourceModel) {
        visit_source_model_mut(self, i);
    }

    /// Visits a [`SourceTable`] node mutably.
    ///
    /// The default implementation delegates to [`visit_source_table_mut`].
    fn visit_source_table_mut(&mut self, i: &mut SourceTable) {
        visit_source_table_mut(self, i);
    }

    /// Visits a [`SourceTableId`] node mutably.
    ///
    /// The default implementation delegates to [`visit_source_table_id_mut`].
    fn visit_source_table_id_mut(&mut self, i: &mut SourceTableId) {
        visit_source_table_id_mut(self, i);
    }

    /// Visits a [`Statement`] node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_mut`].
    fn visit_stmt_mut(&mut self, i: &mut Statement) {
        visit_stmt_mut(self, i);
    }

    /// Visits a [`Delete`] statement node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_delete_mut`].
    fn visit_stmt_delete_mut(&mut self, i: &mut Delete) {
        visit_stmt_delete_mut(self, i);
    }

    /// Visits an [`Insert`] statement node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_insert_mut`].
    fn visit_stmt_insert_mut(&mut self, i: &mut Insert) {
        visit_stmt_insert_mut(self, i);
    }

    /// Visits a [`Query`] statement node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_query_mut`].
    fn visit_stmt_query_mut(&mut self, i: &mut Query) {
        visit_stmt_query_mut(self, i);
    }

    /// Visits a [`Select`] statement node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_select_mut`].
    fn visit_stmt_select_mut(&mut self, i: &mut Select) {
        visit_stmt_select_mut(self, i);
    }

    /// Visits an [`Update`] statement node mutably.
    ///
    /// The default implementation delegates to [`visit_stmt_update_mut`].
    fn visit_stmt_update_mut(&mut self, i: &mut Update) {
        visit_stmt_update_mut(self, i);
    }

    /// Visits a [`TableDerived`] node mutably.
    ///
    /// The default implementation delegates to [`visit_table_derived_mut`].
    fn visit_table_derived_mut(&mut self, i: &mut TableDerived) {
        visit_table_derived_mut(self, i);
    }

    /// Visits a [`TableRef`] node mutably.
    ///
    /// The default implementation delegates to [`visit_table_ref_mut`].
    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        visit_table_ref_mut(self, i);
    }

    /// Visits a [`TableFactor`] node mutably.
    ///
    /// The default implementation delegates to [`visit_table_factor_mut`].
    fn visit_table_factor_mut(&mut self, i: &mut TableFactor) {
        visit_table_factor_mut(self, i);
    }

    /// Visits a [`TableWithJoins`] node mutably.
    ///
    /// The default implementation delegates to [`visit_table_with_joins_mut`].
    fn visit_table_with_joins_mut(&mut self, i: &mut TableWithJoins) {
        visit_table_with_joins_mut(self, i);
    }

    /// Visits a [`Type`] node mutably.
    ///
    /// The default implementation delegates to [`visit_type_mut`].
    fn visit_type_mut(&mut self, i: &mut Type) {
        visit_type_mut(self, i);
    }

    /// Visits an [`UpdateTarget`] node mutably.
    ///
    /// The default implementation delegates to [`visit_update_target_mut`].
    fn visit_update_target_mut(&mut self, i: &mut UpdateTarget) {
        visit_update_target_mut(self, i);
    }

    /// Visits a [`Value`] node mutably.
    ///
    /// The default implementation delegates to [`visit_value_mut`].
    fn visit_value_mut(&mut self, i: &mut Value) {
        visit_value_mut(self, i);
    }

    /// Visits a [`ValueRecord`] node mutably.
    ///
    /// The default implementation delegates to [`visit_value_record`].
    fn visit_value_record(&mut self, i: &mut ValueRecord) {
        visit_value_record(self, i);
    }

    /// Visits a [`Values`] node mutably.
    ///
    /// The default implementation delegates to [`visit_values_mut`].
    fn visit_values_mut(&mut self, i: &mut Values) {
        visit_values_mut(self, i);
    }

    /// Visits a [`With`] node mutably.
    ///
    /// The default implementation delegates to [`visit_with_mut`].
    fn visit_with_mut(&mut self, i: &mut With) {
        visit_with_mut(self, i);
    }
}

impl<V: VisitMut> VisitMut for &mut V {
    fn visit_assignment_mut(&mut self, i: &mut Assignment) {
        VisitMut::visit_assignment_mut(&mut **self, i);
    }

    fn visit_assignments_mut(&mut self, i: &mut Assignments) {
        VisitMut::visit_assignments_mut(&mut **self, i);
    }

    fn visit_association_mut(&mut self, i: &mut Association) {
        VisitMut::visit_association_mut(&mut **self, i);
    }

    fn visit_cte_mut(&mut self, i: &mut Cte) {
        VisitMut::visit_cte_mut(&mut **self, i);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        VisitMut::visit_expr_mut(&mut **self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd) {
        VisitMut::visit_expr_and_mut(&mut **self, i);
    }

    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        VisitMut::visit_expr_arg_mut(&mut **self, i);
    }

    fn visit_expr_begins_with_mut(&mut self, i: &mut ExprBeginsWith) {
        VisitMut::visit_expr_begins_with_mut(&mut **self, i);
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp) {
        VisitMut::visit_expr_binary_op_mut(&mut **self, i);
    }

    fn visit_expr_cast_mut(&mut self, i: &mut ExprCast) {
        VisitMut::visit_expr_cast_mut(&mut **self, i);
    }

    fn visit_expr_column_mut(&mut self, i: &mut ExprColumn) {
        VisitMut::visit_expr_column_mut(&mut **self, i);
    }

    fn visit_expr_default_mut(&mut self) {
        VisitMut::visit_expr_default_mut(&mut **self);
    }

    fn visit_expr_error_mut(&mut self, i: &mut ExprError) {
        VisitMut::visit_expr_error_mut(&mut **self, i);
    }

    fn visit_expr_exists_mut(&mut self, i: &mut ExprExists) {
        VisitMut::visit_expr_exists_mut(&mut **self, i);
    }

    fn visit_expr_func_mut(&mut self, i: &mut ExprFunc) {
        VisitMut::visit_expr_func_mut(&mut **self, i);
    }

    fn visit_expr_func_count_mut(&mut self, i: &mut FuncCount) {
        VisitMut::visit_expr_func_count_mut(&mut **self, i);
    }

    fn visit_expr_in_list_mut(&mut self, i: &mut ExprInList) {
        VisitMut::visit_expr_in_list_mut(&mut **self, i);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery) {
        VisitMut::visit_expr_in_subquery_mut(&mut **self, i);
    }

    fn visit_expr_is_null_mut(&mut self, i: &mut ExprIsNull) {
        VisitMut::visit_expr_is_null_mut(&mut **self, i);
    }

    fn visit_expr_is_variant_mut(&mut self, i: &mut ExprIsVariant) {
        VisitMut::visit_expr_is_variant_mut(&mut **self, i);
    }

    fn visit_expr_let_mut(&mut self, i: &mut ExprLet) {
        VisitMut::visit_expr_let_mut(&mut **self, i);
    }

    fn visit_expr_map_mut(&mut self, i: &mut ExprMap) {
        VisitMut::visit_expr_map_mut(&mut **self, i);
    }

    fn visit_expr_match_mut(&mut self, i: &mut ExprMatch) {
        VisitMut::visit_expr_match_mut(&mut **self, i);
    }

    fn visit_expr_not_mut(&mut self, i: &mut ExprNot) {
        VisitMut::visit_expr_not_mut(&mut **self, i);
    }

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr) {
        VisitMut::visit_expr_or_mut(&mut **self, i);
    }

    fn visit_expr_list_mut(&mut self, i: &mut ExprList) {
        VisitMut::visit_expr_list_mut(&mut **self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord) {
        VisitMut::visit_expr_record_mut(&mut **self, i);
    }

    fn visit_expr_reference_mut(&mut self, i: &mut ExprReference) {
        VisitMut::visit_expr_reference_mut(&mut **self, i);
    }

    fn visit_expr_set_mut(&mut self, i: &mut ExprSet) {
        VisitMut::visit_expr_set_mut(&mut **self, i);
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp) {
        VisitMut::visit_expr_set_op_mut(&mut **self, i);
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt) {
        VisitMut::visit_expr_stmt_mut(&mut **self, i);
    }

    fn visit_filter_mut(&mut self, i: &mut Filter) {
        VisitMut::visit_filter_mut(&mut **self, i);
    }

    fn visit_condition_mut(&mut self, i: &mut Condition) {
        VisitMut::visit_condition_mut(&mut **self, i);
    }

    fn visit_expr_project_mut(&mut self, i: &mut ExprProject) {
        VisitMut::visit_expr_project_mut(&mut **self, i);
    }

    fn visit_insert_target_mut(&mut self, i: &mut InsertTarget) {
        VisitMut::visit_insert_target_mut(&mut **self, i);
    }

    fn visit_join_mut(&mut self, i: &mut Join) {
        VisitMut::visit_join_mut(&mut **self, i);
    }

    fn visit_limit_mut(&mut self, i: &mut Limit) {
        VisitMut::visit_limit_mut(&mut **self, i);
    }

    fn visit_limit_cursor_mut(&mut self, i: &mut LimitCursor) {
        VisitMut::visit_limit_cursor_mut(&mut **self, i);
    }

    fn visit_limit_offset_mut(&mut self, i: &mut LimitOffset) {
        VisitMut::visit_limit_offset_mut(&mut **self, i);
    }

    fn visit_order_by_mut(&mut self, i: &mut OrderBy) {
        VisitMut::visit_order_by_mut(&mut **self, i);
    }

    fn visit_order_by_expr_mut(&mut self, i: &mut OrderByExpr) {
        VisitMut::visit_order_by_expr_mut(&mut **self, i);
    }

    fn visit_path_mut(&mut self, i: &mut Path) {
        VisitMut::visit_path_mut(&mut **self, i);
    }

    fn visit_projection_mut(&mut self, i: &mut Projection) {
        VisitMut::visit_projection_mut(&mut **self, i);
    }

    fn visit_returning_mut(&mut self, i: &mut Returning) {
        VisitMut::visit_returning_mut(&mut **self, i);
    }

    fn visit_source_mut(&mut self, i: &mut Source) {
        VisitMut::visit_source_mut(&mut **self, i);
    }

    fn visit_source_model_mut(&mut self, i: &mut SourceModel) {
        VisitMut::visit_source_model_mut(&mut **self, i);
    }

    fn visit_source_table_mut(&mut self, i: &mut SourceTable) {
        VisitMut::visit_source_table_mut(&mut **self, i);
    }

    fn visit_source_table_id_mut(&mut self, i: &mut SourceTableId) {
        VisitMut::visit_source_table_id_mut(&mut **self, i);
    }

    fn visit_stmt_mut(&mut self, i: &mut Statement) {
        VisitMut::visit_stmt_mut(&mut **self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete) {
        VisitMut::visit_stmt_delete_mut(&mut **self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert) {
        VisitMut::visit_stmt_insert_mut(&mut **self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query) {
        VisitMut::visit_stmt_query_mut(&mut **self, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut Select) {
        VisitMut::visit_stmt_select_mut(&mut **self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update) {
        VisitMut::visit_stmt_update_mut(&mut **self, i);
    }

    fn visit_table_derived_mut(&mut self, i: &mut TableDerived) {
        VisitMut::visit_table_derived_mut(&mut **self, i);
    }

    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        VisitMut::visit_table_ref_mut(&mut **self, i);
    }

    fn visit_table_factor_mut(&mut self, i: &mut TableFactor) {
        VisitMut::visit_table_factor_mut(&mut **self, i);
    }

    fn visit_table_with_joins_mut(&mut self, i: &mut TableWithJoins) {
        VisitMut::visit_table_with_joins_mut(&mut **self, i);
    }

    fn visit_type_mut(&mut self, i: &mut Type) {
        VisitMut::visit_type_mut(&mut **self, i);
    }

    fn visit_update_target_mut(&mut self, i: &mut UpdateTarget) {
        VisitMut::visit_update_target_mut(&mut **self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value) {
        VisitMut::visit_value_mut(&mut **self, i);
    }

    fn visit_value_record(&mut self, i: &mut ValueRecord) {
        VisitMut::visit_value_record(&mut **self, i);
    }

    fn visit_values_mut(&mut self, i: &mut Values) {
        VisitMut::visit_values_mut(&mut **self, i);
    }

    fn visit_with_mut(&mut self, i: &mut With) {
        VisitMut::visit_with_mut(&mut **self, i);
    }
}

/// Default mutable traversal for [`Assignment`] nodes. Visits the assignment's expression(s).
pub fn visit_assignment_mut<V>(v: &mut V, node: &mut Assignment)
where
    V: VisitMut + ?Sized,
{
    match node {
        Assignment::Set(expr) | Assignment::Insert(expr) | Assignment::Remove(expr) => {
            v.visit_expr_mut(expr);
        }
        Assignment::Batch(entries) => {
            for entry in entries {
                visit_assignment_mut(v, entry);
            }
        }
    }
}

/// Default mutable traversal for [`Assignments`] nodes. Visits each assignment.
pub fn visit_assignments_mut<V>(v: &mut V, node: &mut Assignments)
where
    V: VisitMut + ?Sized,
{
    for (_, assignment) in node.iter_mut() {
        v.visit_assignment_mut(assignment);
    }
}

/// Default mutable traversal for [`Association`] nodes. Visits the source query.
pub fn visit_association_mut<V>(v: &mut V, node: &mut Association)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.source);
}

/// Default mutable traversal for [`Cte`] nodes. Visits the CTE's query.
pub fn visit_cte_mut<V>(v: &mut V, node: &mut Cte)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.query);
}

/// Default mutable traversal for [`Expr`] nodes. Dispatches to variant-specific visitor.
pub fn visit_expr_mut<V>(v: &mut V, node: &mut Expr)
where
    V: VisitMut + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and_mut(expr),
        Expr::Any(expr) => v.visit_expr_any_mut(expr),
        Expr::Arg(expr) => v.visit_expr_arg_mut(expr),
        Expr::BeginsWith(expr) => v.visit_expr_begins_with_mut(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op_mut(expr),
        Expr::Cast(expr) => v.visit_expr_cast_mut(expr),
        Expr::Default => v.visit_expr_default_mut(),
        Expr::Error(expr) => v.visit_expr_error_mut(expr),
        Expr::Exists(expr) => v.visit_expr_exists_mut(expr),
        Expr::Func(expr) => v.visit_expr_func_mut(expr),
        Expr::Ident(_) => {}
        Expr::InList(expr) => v.visit_expr_in_list_mut(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery_mut(expr),
        Expr::IsNull(expr) => v.visit_expr_is_null_mut(expr),
        Expr::IsVariant(expr) => v.visit_expr_is_variant_mut(expr),
        Expr::Let(expr) => v.visit_expr_let_mut(expr),
        Expr::Map(expr) => v.visit_expr_map_mut(expr),
        Expr::Match(expr) => v.visit_expr_match_mut(expr),
        Expr::Not(expr) => v.visit_expr_not_mut(expr),
        Expr::Or(expr) => v.visit_expr_or_mut(expr),
        Expr::Project(expr) => v.visit_expr_project_mut(expr),
        Expr::Record(expr) => v.visit_expr_record_mut(expr),
        Expr::Reference(expr) => v.visit_expr_reference_mut(expr),
        Expr::List(expr) => v.visit_expr_list_mut(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt_mut(expr),
        Expr::Value(expr) => v.visit_value_mut(expr),
    }
}

/// Default mutable traversal for [`ExprAnd`] nodes. Visits each operand.
pub fn visit_expr_and_mut<V>(v: &mut V, node: &mut ExprAnd)
where
    V: VisitMut + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`ExprAny`] nodes. Visits the inner expression.
pub fn visit_expr_any_mut<V>(v: &mut V, node: &mut ExprAny)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

/// Default mutable traversal for [`ExprArg`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_arg_mut<V>(v: &mut V, node: &mut ExprArg)
where
    V: VisitMut + ?Sized,
{
}

/// Default mutable traversal for [`ExprBeginsWith`] nodes. Visits the attribute expression and prefix.
pub fn visit_expr_begins_with_mut<V>(v: &mut V, node: &mut ExprBeginsWith)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.prefix);
}

/// Default mutable traversal for [`ExprBinaryOp`] nodes. Visits the lhs and rhs expressions.
pub fn visit_expr_binary_op_mut<V>(v: &mut V, node: &mut ExprBinaryOp)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.lhs);
    v.visit_expr_mut(&mut node.rhs);
}

/// Default mutable traversal for [`ExprCast`] nodes. Visits the expression and its target type.
pub fn visit_expr_cast_mut<V>(v: &mut V, node: &mut ExprCast)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_type_mut(&mut node.ty);
}

/// Default mutable traversal for [`ExprColumn`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_column_mut<V>(v: &mut V, node: &mut ExprColumn)
where
    V: VisitMut + ?Sized,
{
}

/// Default mutable traversal for [`Expr::Default`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_default_mut<V>(v: &mut V)
where
    V: VisitMut + ?Sized,
{
}

/// Default mutable traversal for [`ExprError`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_error_mut<V>(v: &mut V, node: &mut ExprError)
where
    V: VisitMut + ?Sized,
{
    // ExprError has no child expressions to visit
}

/// Default mutable traversal for [`ExprExists`] nodes. Visits the subquery.
pub fn visit_expr_exists_mut<V>(v: &mut V, node: &mut ExprExists)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.subquery);
}

/// Default mutable traversal for [`ExprFunc`] nodes. Dispatches to the function-specific visitor.
pub fn visit_expr_func_mut<V>(v: &mut V, node: &mut ExprFunc)
where
    V: VisitMut + ?Sized,
{
    match node {
        ExprFunc::Count(func) => v.visit_expr_func_count_mut(func),
        ExprFunc::LastInsertId(func) => v.visit_expr_func_last_insert_id_mut(func),
    }
}

/// Default mutable traversal for [`FuncCount`] nodes. Visits the optional argument and optional filter expressions.
pub fn visit_expr_func_count_mut<V>(v: &mut V, node: &mut FuncCount)
where
    V: VisitMut + ?Sized,
{
    if let Some(expr) = &mut node.arg {
        v.visit_expr_mut(expr);
    }

    if let Some(expr) = &mut node.filter {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`FuncLastInsertId`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_func_last_insert_id_mut<V>(_v: &mut V, _node: &mut FuncLastInsertId)
where
    V: VisitMut + ?Sized,
{
    // FuncLastInsertId has no fields to visit
}

/// Default mutable traversal for [`ExprInList`] nodes. Visits the expression and the list expression.
pub fn visit_expr_in_list_mut<V>(v: &mut V, node: &mut ExprInList)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.list);
}

/// Default mutable traversal for [`ExprInSubquery`] nodes. Visits the expression and the subquery.
pub fn visit_expr_in_subquery_mut<V>(v: &mut V, node: &mut ExprInSubquery)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_stmt_query_mut(&mut node.query);
}

/// Default mutable traversal for [`ExprIsNull`] nodes. Visits the inner expression.
pub fn visit_expr_is_null_mut<V>(v: &mut V, node: &mut ExprIsNull)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

/// Default mutable traversal for [`ExprIsVariant`] nodes. Visits the inner expression.
pub fn visit_expr_is_variant_mut<V>(v: &mut V, node: &mut ExprIsVariant)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

/// Default mutable traversal for [`ExprLet`] nodes. Visits each binding expression and the body.
pub fn visit_expr_let_mut<V>(v: &mut V, node: &mut ExprLet)
where
    V: VisitMut + ?Sized,
{
    for binding in &mut node.bindings {
        v.visit_expr_mut(binding);
    }
    v.visit_expr_mut(&mut node.body);
}

/// Default mutable traversal for [`ExprMap`] nodes. Visits the base expression and the map expression.
pub fn visit_expr_map_mut<V>(v: &mut V, node: &mut ExprMap)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_expr_mut(&mut node.map);
}

/// Default mutable traversal for [`ExprMatch`] nodes. Visits the subject, each arm's expression, and the else expression.
pub fn visit_expr_match_mut<V>(v: &mut V, node: &mut ExprMatch)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.subject);
    for arm in &mut node.arms {
        v.visit_expr_mut(&mut arm.expr);
    }
    v.visit_expr_mut(&mut node.else_expr);
}

/// Default mutable traversal for [`ExprNot`] nodes. Visits the inner expression.
pub fn visit_expr_not_mut<V>(v: &mut V, node: &mut ExprNot)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

/// Default mutable traversal for [`ExprOr`] nodes. Visits each operand.
pub fn visit_expr_or_mut<V>(v: &mut V, node: &mut ExprOr)
where
    V: VisitMut + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`ExprList`] nodes. Visits each item expression.
pub fn visit_expr_list_mut<V>(v: &mut V, node: &mut ExprList)
where
    V: VisitMut + ?Sized,
{
    for e in &mut node.items {
        v.visit_expr_mut(e);
    }
}

/// Default mutable traversal for [`ExprRecord`] nodes. Visits each field expression.
pub fn visit_expr_record_mut<V>(v: &mut V, node: &mut ExprRecord)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut **node {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`ExprReference`] nodes. Dispatches based on the reference kind.
pub fn visit_expr_reference_mut<V>(v: &mut V, node: &mut ExprReference)
where
    V: VisitMut + ?Sized,
{
    match node {
        ExprReference::Model { .. } => {}
        ExprReference::Field { .. } => {}
        ExprReference::Column(expr_column) => v.visit_expr_column_mut(expr_column),
    }
}

/// Default mutable traversal for [`ExprSet`] nodes. Dispatches to the set expression variant visitor.
pub fn visit_expr_set_mut<V>(v: &mut V, node: &mut ExprSet)
where
    V: VisitMut + ?Sized,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select_mut(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op_mut(expr),
        ExprSet::Update(expr) => v.visit_stmt_update_mut(expr),
        ExprSet::Values(expr) => v.visit_values_mut(expr),
        ExprSet::Insert(expr) => v.visit_stmt_insert_mut(expr),
    }
}

/// Default mutable traversal for [`ExprSetOp`] nodes. Visits each operand.
pub fn visit_expr_set_op_mut<V>(v: &mut V, node: &mut ExprSetOp)
where
    V: VisitMut + ?Sized,
{
    for operand in &mut node.operands {
        v.visit_expr_set_mut(operand);
    }
}

/// Default mutable traversal for [`ExprStmt`] nodes. Visits the inner statement.
pub fn visit_expr_stmt_mut<V>(v: &mut V, node: &mut ExprStmt)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_mut(&mut node.stmt);
}

/// Default mutable traversal for [`ExprProject`] nodes. Visits the base expression and the projection.
pub fn visit_expr_project_mut<V>(v: &mut V, node: &mut ExprProject)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_projection_mut(&mut node.projection);
}

/// Default mutable traversal for [`Filter`] nodes. Visits the optional filter expression.
pub fn visit_filter_mut<V>(v: &mut V, node: &mut Filter)
where
    V: VisitMut + ?Sized,
{
    if let Some(expr) = &mut node.expr {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`Condition`] nodes. Visits the optional condition expression.
pub fn visit_condition_mut<V>(v: &mut V, node: &mut Condition)
where
    V: VisitMut + ?Sized,
{
    if let Some(expr) = &mut node.expr {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`InsertTarget`] nodes. Visits the scope query if present.
pub fn visit_insert_target_mut<V>(v: &mut V, node: &mut InsertTarget)
where
    V: VisitMut + ?Sized,
{
    if let InsertTarget::Scope(stmt) = node {
        v.visit_stmt_query_mut(stmt)
    }
}

/// Default mutable traversal for [`Join`] nodes. Visits the table and the join constraint expression.
pub fn visit_join_mut<V>(v: &mut V, node: &mut Join)
where
    V: VisitMut + ?Sized,
{
    v.visit_source_table_id_mut(&mut node.table);
    match &mut node.constraint {
        JoinOp::Left(expr) => v.visit_expr_mut(expr),
    }
}

/// Default mutable traversal for [`Limit`] nodes.
pub fn visit_limit_mut<V>(v: &mut V, node: &mut Limit)
where
    V: VisitMut + ?Sized,
{
    match node {
        Limit::Cursor(cursor) => v.visit_limit_cursor_mut(cursor),
        Limit::Offset(offset) => v.visit_limit_offset_mut(offset),
    }
}

/// Default mutable traversal for [`LimitCursor`] nodes.
pub fn visit_limit_cursor_mut<V>(v: &mut V, node: &mut LimitCursor)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.page_size);
    if let Some(after) = &mut node.after {
        v.visit_expr_mut(after);
    }
}

/// Default mutable traversal for [`LimitOffset`] nodes.
pub fn visit_limit_offset_mut<V>(v: &mut V, node: &mut LimitOffset)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.limit);
    if let Some(offset) = &mut node.offset {
        v.visit_expr_mut(offset);
    }
}

/// Default mutable traversal for [`OrderBy`] nodes. Visits each ordering expression.
pub fn visit_order_by_mut<V>(v: &mut V, node: &mut OrderBy)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.exprs {
        v.visit_order_by_expr_mut(expr);
    }
}

/// Default mutable traversal for [`OrderByExpr`] nodes. Visits the ordering expression.
pub fn visit_order_by_expr_mut<V>(v: &mut V, node: &mut OrderByExpr)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

/// Default mutable traversal for [`Path`] nodes. Visits the projection.
pub fn visit_path_mut<V>(v: &mut V, node: &mut Path)
where
    V: VisitMut + ?Sized,
{
    v.visit_projection_mut(&mut node.projection);
}

/// Default mutable traversal for [`Projection`] nodes. This is a leaf node with no children to visit.
pub fn visit_projection_mut<V>(v: &mut V, node: &mut Projection)
where
    V: VisitMut + ?Sized,
{
}

/// Default mutable traversal for [`Returning`] nodes. Visits each path for model variants, or the expression for expr and value variants.
pub fn visit_returning_mut<V>(v: &mut V, node: &mut Returning)
where
    V: VisitMut + ?Sized,
{
    match node {
        Returning::Model { include } => {
            for path in include {
                v.visit_path_mut(path);
            }
        }
        Returning::Changed => {}
        Returning::Expr(expr) => v.visit_expr_mut(expr),
        Returning::Value(expr) => v.visit_expr_mut(expr),
    }
}

/// Default mutable traversal for [`Source`] nodes. Dispatches to the model or table source visitor.
pub fn visit_source_mut<V>(v: &mut V, node: &mut Source)
where
    V: VisitMut + ?Sized,
{
    match node {
        Source::Model(source_model) => v.visit_source_model_mut(source_model),
        Source::Table(source_table) => v.visit_source_table_mut(source_table),
    }
}

/// Default mutable traversal for [`SourceModel`] nodes. Visits the optional association.
pub fn visit_source_model_mut<V>(v: &mut V, node: &mut SourceModel)
where
    V: VisitMut + ?Sized,
{
    if let Some(association) = &mut node.via {
        v.visit_association_mut(association);
    }
}

/// Default mutable traversal for [`SourceTable`] nodes. Visits each table reference and each FROM clause table-with-joins.
pub fn visit_source_table_mut<V>(v: &mut V, node: &mut SourceTable)
where
    V: VisitMut + ?Sized,
{
    for table_ref in &mut node.tables {
        v.visit_table_ref_mut(table_ref);
    }
    for table_with_joins in &mut node.from {
        v.visit_table_with_joins_mut(table_with_joins);
    }
}

/// Default mutable traversal for [`SourceTableId`] nodes. This is a leaf node with no children to visit.
pub fn visit_source_table_id_mut<V>(v: &mut V, node: &mut SourceTableId)
where
    V: VisitMut + ?Sized,
{
    // SourceTableId is just an index, nothing to visit
}

/// Default mutable traversal for [`TableFactor`] nodes. Dispatches by factor type.
pub fn visit_table_factor_mut<V>(v: &mut V, node: &mut TableFactor)
where
    V: VisitMut + ?Sized,
{
    match node {
        TableFactor::Table(table_id) => v.visit_source_table_id_mut(table_id),
    }
}

/// Default mutable traversal for [`Statement`] nodes. Dispatches to the statement-specific visitor.
pub fn visit_stmt_mut<V>(v: &mut V, node: &mut Statement)
where
    V: VisitMut + ?Sized,
{
    match node {
        Statement::Delete(stmt) => v.visit_stmt_delete_mut(stmt),
        Statement::Insert(stmt) => v.visit_stmt_insert_mut(stmt),
        Statement::Query(stmt) => v.visit_stmt_query_mut(stmt),
        Statement::Update(stmt) => v.visit_stmt_update_mut(stmt),
    }
}

/// Default mutable traversal for [`Delete`] nodes. Visits the source, filter, and optional returning clause.
pub fn visit_stmt_delete_mut<V>(v: &mut V, node: &mut Delete)
where
    V: VisitMut + ?Sized,
{
    v.visit_source_mut(&mut node.from);
    v.visit_filter_mut(&mut node.filter);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

/// Default mutable traversal for [`Insert`] nodes. Visits the target, source query, and optional returning clause.
pub fn visit_stmt_insert_mut<V>(v: &mut V, node: &mut Insert)
where
    V: VisitMut + ?Sized,
{
    v.visit_insert_target_mut(&mut node.target);
    v.visit_stmt_query_mut(&mut node.source);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

/// Default mutable traversal for [`Query`] nodes. Visits the optional WITH clause, body, optional ORDER BY, and optional LIMIT.
pub fn visit_stmt_query_mut<V>(v: &mut V, node: &mut Query)
where
    V: VisitMut + ?Sized,
{
    if let Some(with) = &mut node.with {
        v.visit_with_mut(with);
    }

    v.visit_expr_set_mut(&mut node.body);

    if let Some(order_by) = &mut node.order_by {
        v.visit_order_by_mut(order_by);
    }

    if let Some(limit) = &mut node.limit {
        v.visit_limit_mut(limit);
    }
}

/// Default mutable traversal for [`Select`] nodes. Visits the source, filter, and returning clause.
pub fn visit_stmt_select_mut<V>(v: &mut V, node: &mut Select)
where
    V: VisitMut + ?Sized,
{
    v.visit_source_mut(&mut node.source);
    v.visit_filter_mut(&mut node.filter);
    v.visit_returning_mut(&mut node.returning);
}

/// Default mutable traversal for [`Update`] nodes. Visits the target, assignments, filter, condition, and optional returning clause.
pub fn visit_stmt_update_mut<V>(v: &mut V, node: &mut Update)
where
    V: VisitMut + ?Sized,
{
    v.visit_update_target_mut(&mut node.target);
    v.visit_assignments_mut(&mut node.assignments);
    v.visit_filter_mut(&mut node.filter);
    v.visit_condition_mut(&mut node.condition);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

/// Default mutable traversal for [`TableDerived`] nodes. Visits the subquery.
pub fn visit_table_derived_mut<V>(v: &mut V, node: &mut TableDerived)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.subquery);
}

/// Default mutable traversal for [`TableRef`] nodes. Dispatches by reference kind, visiting derived tables and arg expressions.
pub fn visit_table_ref_mut<V>(v: &mut V, node: &mut TableRef)
where
    V: VisitMut + ?Sized,
{
    match node {
        TableRef::Cte { .. } => {}
        TableRef::Derived(table_derived) => v.visit_table_derived_mut(table_derived),
        TableRef::Table(_) => {}
        TableRef::Arg(expr_arg) => v.visit_expr_arg_mut(expr_arg),
    }
}

/// Default mutable traversal for [`TableWithJoins`] nodes. Visits the relation and each join.
pub fn visit_table_with_joins_mut<V>(v: &mut V, node: &mut TableWithJoins)
where
    V: VisitMut + ?Sized,
{
    v.visit_table_factor_mut(&mut node.relation);
    for join in &mut node.joins {
        v.visit_join_mut(join);
    }
}

/// Default mutable traversal for [`Type`] nodes. This is a leaf node with no children to visit.
pub fn visit_type_mut<V>(v: &mut V, node: &mut Type)
where
    V: VisitMut + ?Sized,
{
    // Type is just type information, no traversal needed
}

/// Default mutable traversal for [`UpdateTarget`] nodes. Visits the query if present.
pub fn visit_update_target_mut<V>(v: &mut V, node: &mut UpdateTarget)
where
    V: VisitMut + ?Sized,
{
    if let UpdateTarget::Query(stmt) = node {
        v.visit_stmt_query_mut(stmt)
    }
}

/// Default mutable traversal for [`Value`] nodes. Visits the inner record if the value is a record variant.
pub fn visit_value_mut<V>(v: &mut V, node: &mut Value)
where
    V: VisitMut + ?Sized,
{
    if let Value::Record(node) = node {
        v.visit_value_record(node);
    }
}

/// Default mutable traversal for [`ValueRecord`] nodes. Visits each value field.
pub fn visit_value_record<V>(v: &mut V, node: &mut ValueRecord)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.fields {
        v.visit_value_mut(expr);
    }
}

/// Default mutable traversal for [`Values`] nodes. Visits each row expression.
pub fn visit_values_mut<V>(v: &mut V, node: &mut Values)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.rows {
        v.visit_expr_mut(expr);
    }
}

/// Default mutable traversal for [`With`] nodes. Visits each CTE.
pub fn visit_with_mut<V>(v: &mut V, node: &mut With)
where
    V: VisitMut + ?Sized,
{
    for cte in &mut node.ctes {
        v.visit_cte_mut(cte);
    }
}

/// Calls `f` for every [`Expr`] node reachable from `node`, in post-order,
/// allowing mutation.
pub fn for_each_expr_mut<F>(node: &mut impl Node, f: F)
where
    F: FnMut(&mut Expr),
{
    struct ForEach<F> {
        f: F,
    }

    impl<F> VisitMut for ForEach<F>
    where
        F: FnMut(&mut Expr),
    {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            visit_expr_mut(self, node);
            (self.f)(node);
        }
    }

    node.visit_mut(ForEach { f });
}

/// Walk an expression tree in pre-order, tracking scope depth through
/// Let/Map scopes.
///
/// For each node, calls `f(expr, scope_depth)`:
/// - If `f` returns `true`, recursion into children continues.
/// - If `f` returns `false`, children are skipped (e.g., when the callback
///   has replaced the expression and doesn't want to recurse into the
///   replacement).
///
/// Scope depth rules:
/// - `Let` bindings are visited at the current depth; the body at `depth + 1`
/// - `Map` base is visited at the current depth; the map function at `depth + 1`
/// - All other compound expressions: children at the same depth
///
/// This matches the semantics of `ExprArg.nesting`: an arg with
/// `nesting == scope_depth` references the outermost (statement-level) scope,
/// while `nesting < scope_depth` references a Let/Map binding.
pub fn walk_expr_scoped_mut<F>(expr: &mut Expr, scope_depth: usize, mut f: F)
where
    F: FnMut(&mut Expr, usize) -> bool,
{
    walk_expr_scoped_mut_ref(expr, scope_depth, &mut f);
}

fn walk_expr_scoped_mut_ref<F>(expr: &mut Expr, scope_depth: usize, f: &mut F)
where
    F: FnMut(&mut Expr, usize) -> bool,
{
    struct ScopedWalk<'a, F> {
        f: &'a mut F,
        scope_depth: usize,
    }

    impl<F> VisitMut for ScopedWalk<'_, F>
    where
        F: FnMut(&mut Expr, usize) -> bool,
    {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if !(self.f)(node, self.scope_depth) {
                return;
            }
            visit_expr_mut(self, node);
        }

        fn visit_expr_let_mut(&mut self, node: &mut ExprLet) {
            for binding in &mut node.bindings {
                self.visit_expr_mut(binding);
            }
            self.scope_depth += 1;
            self.visit_expr_mut(&mut node.body);
            self.scope_depth -= 1;
        }

        fn visit_expr_map_mut(&mut self, node: &mut ExprMap) {
            self.visit_expr_mut(&mut node.base);
            self.scope_depth += 1;
            self.visit_expr_mut(&mut node.map);
            self.scope_depth -= 1;
        }
    }

    ScopedWalk { f, scope_depth }.visit_expr_mut(expr);
}
