#![allow(unused_variables)]

use super::{
    Assignment, Assignments, Association, Condition, Cte, Delete, Expr, ExprAnd, ExprAny, ExprArg,
    ExprBeginsWith, ExprBinaryOp, ExprCast, ExprColumn, ExprError, ExprExists, ExprFunc,
    ExprInList, ExprInSubquery, ExprIsNull, ExprIsVariant, ExprLet, ExprLike, ExprList, ExprMap,
    ExprMatch, ExprNot, ExprOr, ExprProject, ExprRecord, ExprReference, ExprSet, ExprSetOp,
    ExprStmt, Filter, FuncCount, FuncLastInsertId, Insert, InsertTarget, Join, JoinOp, Limit,
    LimitCursor, LimitOffset, Node, OrderBy, OrderByExpr, Path, Projection, Query, Returning,
    Select, Source, SourceModel, SourceTable, SourceTableId, Statement, TableDerived, TableFactor,
    TableRef, TableWithJoins, Type, Update, UpdateTarget, Value, ValueRecord, Values, With,
};

/// Immutable visitor trait for the statement AST.
///
/// Implement this trait to walk the AST without modifying it. Each
/// `visit_*` method has a default implementation that recurses into
/// child nodes via the corresponding free function (e.g.,
/// [`visit_expr`]). Override specific methods to inspect nodes of
/// interest.
///
/// The companion [`for_each_expr`] helper visits every expression node
/// in post-order.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{visit, Expr, Value, Node};
///
/// let expr = Expr::from(Value::from(42_i64));
/// let mut count = 0;
/// visit::for_each_expr(&expr, |_| count += 1);
/// assert_eq!(count, 1);
/// ```
pub trait Visit {
    /// Dispatches to the appropriate `visit_*` method via [`Node::visit`].
    fn visit<N: Node>(&mut self, i: &N)
    where
        Self: Sized,
    {
        i.visit(self);
    }

    /// Visits an [`Assignment`] node.
    ///
    /// The default implementation delegates to [`visit_assignment`].
    fn visit_assignment(&mut self, i: &Assignment) {
        visit_assignment(self, i);
    }

    /// Visits an [`Assignments`] node.
    ///
    /// The default implementation delegates to [`visit_assignments`].
    fn visit_assignments(&mut self, i: &Assignments) {
        visit_assignments(self, i);
    }

    /// Visits an [`Association`] node.
    ///
    /// The default implementation delegates to [`visit_association`].
    fn visit_association(&mut self, i: &Association) {
        visit_association(self, i);
    }

    /// Visits a [`Cte`] (common table expression) node.
    ///
    /// The default implementation delegates to [`visit_cte`].
    fn visit_cte(&mut self, i: &Cte) {
        visit_cte(self, i);
    }

    /// Visits an [`Expr`] node.
    ///
    /// The default implementation delegates to [`visit_expr`].
    fn visit_expr(&mut self, i: &Expr) {
        visit_expr(self, i);
    }

    /// Visits an [`ExprAnd`] node.
    ///
    /// The default implementation delegates to [`visit_expr_and`].
    fn visit_expr_and(&mut self, i: &ExprAnd) {
        visit_expr_and(self, i);
    }

    /// Visits an [`ExprAny`] node.
    ///
    /// The default implementation delegates to [`visit_expr_any`].
    fn visit_expr_any(&mut self, i: &ExprAny) {
        visit_expr_any(self, i);
    }

    /// Visits an [`ExprArg`] node.
    ///
    /// The default implementation delegates to [`visit_expr_arg`].
    fn visit_expr_arg(&mut self, i: &ExprArg) {
        visit_expr_arg(self, i);
    }

    /// Visits an [`ExprBeginsWith`] node.
    ///
    /// The default implementation delegates to [`visit_expr_begins_with`].
    fn visit_expr_begins_with(&mut self, i: &ExprBeginsWith) {
        visit_expr_begins_with(self, i);
    }

    /// Visits an [`ExprBinaryOp`] node.
    ///
    /// The default implementation delegates to [`visit_expr_binary_op`].
    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp) {
        visit_expr_binary_op(self, i);
    }

    /// Visits an [`ExprCast`] node.
    ///
    /// The default implementation delegates to [`visit_expr_cast`].
    fn visit_expr_cast(&mut self, i: &ExprCast) {
        visit_expr_cast(self, i);
    }

    /// Visits an [`ExprColumn`] node.
    ///
    /// The default implementation delegates to [`visit_expr_column`].
    fn visit_expr_column(&mut self, i: &ExprColumn) {
        visit_expr_column(self, i);
    }

    /// Visits a default expression (no associated data).
    ///
    /// The default implementation delegates to [`visit_expr_default`].
    fn visit_expr_default(&mut self) {
        visit_expr_default(self);
    }

    /// Visits an [`ExprError`] node.
    ///
    /// The default implementation delegates to [`visit_expr_error`].
    fn visit_expr_error(&mut self, i: &ExprError) {
        visit_expr_error(self, i);
    }

    /// Visits an [`ExprExists`] node.
    ///
    /// The default implementation delegates to [`visit_expr_exists`].
    fn visit_expr_exists(&mut self, i: &ExprExists) {
        visit_expr_exists(self, i);
    }

    /// Visits an [`ExprFunc`] node.
    ///
    /// The default implementation delegates to [`visit_expr_func`].
    fn visit_expr_func(&mut self, i: &ExprFunc) {
        visit_expr_func(self, i);
    }

    /// Visits a [`FuncCount`] node.
    ///
    /// The default implementation delegates to [`visit_expr_func_count`].
    fn visit_expr_func_count(&mut self, i: &FuncCount) {
        visit_expr_func_count(self, i);
    }

    /// Visits a [`FuncLastInsertId`] node.
    ///
    /// The default implementation delegates to [`visit_expr_func_last_insert_id`].
    fn visit_expr_func_last_insert_id(&mut self, i: &FuncLastInsertId) {
        visit_expr_func_last_insert_id(self, i);
    }

    /// Visits an [`ExprInList`] node.
    ///
    /// The default implementation delegates to [`visit_expr_in_list`].
    fn visit_expr_in_list(&mut self, i: &ExprInList) {
        visit_expr_in_list(self, i);
    }

    /// Visits an [`ExprInSubquery`] node.
    ///
    /// The default implementation delegates to [`visit_expr_in_subquery`].
    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery) {
        visit_expr_in_subquery(self, i);
    }

    /// Visits an [`ExprIsNull`] node.
    ///
    /// The default implementation delegates to [`visit_expr_is_null`].
    fn visit_expr_is_null(&mut self, i: &ExprIsNull) {
        visit_expr_is_null(self, i);
    }

    /// Visits an [`ExprIsVariant`] node.
    ///
    /// The default implementation delegates to [`visit_expr_is_variant`].
    fn visit_expr_is_variant(&mut self, i: &ExprIsVariant) {
        visit_expr_is_variant(self, i);
    }

    /// Visits an [`ExprLet`] node.
    ///
    /// The default implementation delegates to [`visit_expr_let`].
    fn visit_expr_let(&mut self, i: &ExprLet) {
        visit_expr_let(self, i);
    }

    /// Visits an [`ExprLike`] node.
    ///
    /// The default implementation delegates to [`visit_expr_like`].
    fn visit_expr_like(&mut self, i: &ExprLike) {
        visit_expr_like(self, i);
    }

    /// Visits an [`ExprMap`] node.
    ///
    /// The default implementation delegates to [`visit_expr_map`].
    fn visit_expr_map(&mut self, i: &ExprMap) {
        visit_expr_map(self, i);
    }

    /// Visits an [`ExprMatch`] node.
    ///
    /// The default implementation delegates to [`visit_expr_match`].
    fn visit_expr_match(&mut self, i: &ExprMatch) {
        visit_expr_match(self, i);
    }

    /// Visits an [`ExprNot`] node.
    ///
    /// The default implementation delegates to [`visit_expr_not`].
    fn visit_expr_not(&mut self, i: &ExprNot) {
        visit_expr_not(self, i);
    }

    /// Visits an [`ExprOr`] node.
    ///
    /// The default implementation delegates to [`visit_expr_or`].
    fn visit_expr_or(&mut self, i: &ExprOr) {
        visit_expr_or(self, i);
    }

    /// Visits an [`ExprList`] node.
    ///
    /// The default implementation delegates to [`visit_expr_list`].
    fn visit_expr_list(&mut self, i: &ExprList) {
        visit_expr_list(self, i);
    }

    /// Visits an [`ExprRecord`] node.
    ///
    /// The default implementation delegates to [`visit_expr_record`].
    fn visit_expr_record(&mut self, i: &ExprRecord) {
        visit_expr_record(self, i);
    }

    /// Visits an [`ExprReference`] node.
    ///
    /// The default implementation delegates to [`visit_expr_reference`].
    fn visit_expr_reference(&mut self, i: &ExprReference) {
        visit_expr_reference(self, i);
    }

    /// Visits an [`ExprSet`] node.
    ///
    /// The default implementation delegates to [`visit_expr_set`].
    fn visit_expr_set(&mut self, i: &ExprSet) {
        visit_expr_set(self, i);
    }

    /// Visits an [`ExprSetOp`] node.
    ///
    /// The default implementation delegates to [`visit_expr_set_op`].
    fn visit_expr_set_op(&mut self, i: &ExprSetOp) {
        visit_expr_set_op(self, i);
    }

    /// Visits an [`ExprStmt`] node.
    ///
    /// The default implementation delegates to [`visit_expr_stmt`].
    fn visit_expr_stmt(&mut self, i: &ExprStmt) {
        visit_expr_stmt(self, i);
    }

    /// Visits a [`Filter`] node.
    ///
    /// The default implementation delegates to [`visit_filter`].
    fn visit_filter(&mut self, i: &Filter) {
        visit_filter(self, i);
    }

    /// Visits a [`Condition`] node.
    ///
    /// The default implementation delegates to [`visit_condition`].
    fn visit_condition(&mut self, i: &Condition) {
        visit_condition(self, i);
    }

    /// Visits an [`ExprProject`] node.
    ///
    /// The default implementation delegates to [`visit_expr_project`].
    fn visit_expr_project(&mut self, i: &ExprProject) {
        visit_expr_project(self, i);
    }

    /// Visits an [`InsertTarget`] node.
    ///
    /// The default implementation delegates to [`visit_insert_target`].
    fn visit_insert_target(&mut self, i: &InsertTarget) {
        visit_insert_target(self, i);
    }

    /// Visits a [`Join`] node.
    ///
    /// The default implementation delegates to [`visit_join`].
    fn visit_join(&mut self, i: &Join) {
        visit_join(self, i);
    }

    /// Visits a [`Limit`] node.
    ///
    /// The default implementation delegates to [`visit_limit`].
    fn visit_limit(&mut self, i: &Limit) {
        visit_limit(self, i);
    }

    /// Visits a [`LimitCursor`] node.
    ///
    /// The default implementation delegates to [`visit_limit_cursor`].
    fn visit_limit_cursor(&mut self, i: &LimitCursor) {
        visit_limit_cursor(self, i);
    }

    /// Visits a [`LimitOffset`] node.
    ///
    /// The default implementation delegates to [`visit_limit_offset`].
    fn visit_limit_offset(&mut self, i: &LimitOffset) {
        visit_limit_offset(self, i);
    }

    /// Visits an [`OrderBy`] node.
    ///
    /// The default implementation delegates to [`visit_order_by`].
    fn visit_order_by(&mut self, i: &OrderBy) {
        visit_order_by(self, i);
    }

    /// Visits an [`OrderByExpr`] node.
    ///
    /// The default implementation delegates to [`visit_order_by_expr`].
    fn visit_order_by_expr(&mut self, i: &OrderByExpr) {
        visit_order_by_expr(self, i);
    }

    /// Visits a [`Path`] node.
    ///
    /// The default implementation delegates to [`visit_path`].
    fn visit_path(&mut self, i: &Path) {
        visit_path(self, i);
    }

    /// Visits a [`Projection`] node.
    ///
    /// The default implementation delegates to [`visit_projection`].
    fn visit_projection(&mut self, i: &Projection) {
        visit_projection(self, i);
    }

    /// Visits a [`Returning`] node.
    ///
    /// The default implementation delegates to [`visit_returning`].
    fn visit_returning(&mut self, i: &Returning) {
        visit_returning(self, i);
    }

    /// Visits a [`Source`] node.
    ///
    /// The default implementation delegates to [`visit_source`].
    fn visit_source(&mut self, i: &Source) {
        visit_source(self, i);
    }

    /// Visits a [`SourceModel`] node.
    ///
    /// The default implementation delegates to [`visit_source_model`].
    fn visit_source_model(&mut self, i: &SourceModel) {
        visit_source_model(self, i);
    }

    /// Visits a [`SourceTable`] node.
    ///
    /// The default implementation delegates to [`visit_source_table`].
    fn visit_source_table(&mut self, i: &SourceTable) {
        visit_source_table(self, i);
    }

    /// Visits a [`SourceTableId`] node.
    ///
    /// The default implementation delegates to [`visit_source_table_id`].
    fn visit_source_table_id(&mut self, i: &SourceTableId) {
        visit_source_table_id(self, i);
    }

    /// Visits a [`Statement`] node.
    ///
    /// The default implementation delegates to [`visit_stmt`].
    fn visit_stmt(&mut self, i: &Statement) {
        visit_stmt(self, i);
    }

    /// Visits a [`Delete`] statement node.
    ///
    /// The default implementation delegates to [`visit_stmt_delete`].
    fn visit_stmt_delete(&mut self, i: &Delete) {
        visit_stmt_delete(self, i);
    }

    /// Visits an [`Insert`] statement node.
    ///
    /// The default implementation delegates to [`visit_stmt_insert`].
    fn visit_stmt_insert(&mut self, i: &Insert) {
        visit_stmt_insert(self, i);
    }

    /// Visits a [`Query`] statement node.
    ///
    /// The default implementation delegates to [`visit_stmt_query`].
    fn visit_stmt_query(&mut self, i: &Query) {
        visit_stmt_query(self, i);
    }

    /// Visits a [`Select`] statement node.
    ///
    /// The default implementation delegates to [`visit_stmt_select`].
    fn visit_stmt_select(&mut self, i: &Select) {
        visit_stmt_select(self, i);
    }

    /// Visits an [`Update`] statement node.
    ///
    /// The default implementation delegates to [`visit_stmt_update`].
    fn visit_stmt_update(&mut self, i: &Update) {
        visit_stmt_update(self, i);
    }

    /// Visits a [`TableDerived`] node.
    ///
    /// The default implementation delegates to [`visit_table_derived`].
    fn visit_table_derived(&mut self, i: &TableDerived) {
        visit_table_derived(self, i);
    }

    /// Visits a [`TableRef`] node.
    ///
    /// The default implementation delegates to [`visit_table_ref`].
    fn visit_table_ref(&mut self, i: &TableRef) {
        visit_table_ref(self, i);
    }

    /// Visits a [`TableFactor`] node.
    ///
    /// The default implementation delegates to [`visit_table_factor`].
    fn visit_table_factor(&mut self, i: &TableFactor) {
        visit_table_factor(self, i);
    }

    /// Visits a [`TableWithJoins`] node.
    ///
    /// The default implementation delegates to [`visit_table_with_joins`].
    fn visit_table_with_joins(&mut self, i: &TableWithJoins) {
        visit_table_with_joins(self, i);
    }

    /// Visits a [`Type`] node.
    ///
    /// The default implementation delegates to [`visit_type`].
    fn visit_type(&mut self, i: &Type) {
        visit_type(self, i);
    }

    /// Visits an [`UpdateTarget`] node.
    ///
    /// The default implementation delegates to [`visit_update_target`].
    fn visit_update_target(&mut self, i: &UpdateTarget) {
        visit_update_target(self, i);
    }

    /// Visits a [`Value`] node.
    ///
    /// The default implementation delegates to [`visit_value`].
    fn visit_value(&mut self, i: &Value) {
        visit_value(self, i);
    }

    /// Visits a [`ValueRecord`] node.
    ///
    /// The default implementation delegates to [`visit_value_record`].
    fn visit_value_record(&mut self, i: &ValueRecord) {
        visit_value_record(self, i);
    }

    /// Visits a [`Values`] node.
    ///
    /// The default implementation delegates to [`visit_values`].
    fn visit_values(&mut self, i: &Values) {
        visit_values(self, i);
    }

    /// Visits a [`With`] node.
    ///
    /// The default implementation delegates to [`visit_with`].
    fn visit_with(&mut self, i: &With) {
        visit_with(self, i);
    }
}

impl<V: Visit> Visit for &mut V {
    fn visit_assignment(&mut self, i: &Assignment) {
        Visit::visit_assignment(&mut **self, i);
    }

    fn visit_assignments(&mut self, i: &Assignments) {
        Visit::visit_assignments(&mut **self, i);
    }

    fn visit_association(&mut self, i: &Association) {
        Visit::visit_association(&mut **self, i);
    }

    fn visit_cte(&mut self, i: &Cte) {
        Visit::visit_cte(&mut **self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        Visit::visit_expr(&mut **self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd) {
        Visit::visit_expr_and(&mut **self, i);
    }

    fn visit_expr_arg(&mut self, i: &ExprArg) {
        Visit::visit_expr_arg(&mut **self, i);
    }

    fn visit_expr_begins_with(&mut self, i: &ExprBeginsWith) {
        Visit::visit_expr_begins_with(&mut **self, i);
    }

    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp) {
        Visit::visit_expr_binary_op(&mut **self, i);
    }

    fn visit_expr_cast(&mut self, i: &ExprCast) {
        Visit::visit_expr_cast(&mut **self, i);
    }

    fn visit_expr_column(&mut self, i: &ExprColumn) {
        Visit::visit_expr_column(&mut **self, i);
    }

    fn visit_expr_default(&mut self) {
        Visit::visit_expr_default(&mut **self);
    }

    fn visit_expr_error(&mut self, i: &ExprError) {
        Visit::visit_expr_error(&mut **self, i);
    }

    fn visit_expr_exists(&mut self, i: &ExprExists) {
        Visit::visit_expr_exists(&mut **self, i);
    }

    fn visit_expr_func(&mut self, i: &ExprFunc) {
        Visit::visit_expr_func(&mut **self, i);
    }

    fn visit_expr_func_count(&mut self, i: &FuncCount) {
        Visit::visit_expr_func_count(&mut **self, i);
    }

    fn visit_expr_in_list(&mut self, i: &ExprInList) {
        Visit::visit_expr_in_list(&mut **self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery) {
        Visit::visit_expr_in_subquery(&mut **self, i);
    }

    fn visit_expr_is_null(&mut self, i: &ExprIsNull) {
        Visit::visit_expr_is_null(&mut **self, i);
    }

    fn visit_expr_is_variant(&mut self, i: &ExprIsVariant) {
        Visit::visit_expr_is_variant(&mut **self, i);
    }

    fn visit_expr_let(&mut self, i: &ExprLet) {
        Visit::visit_expr_let(&mut **self, i);
    }

    fn visit_expr_like(&mut self, i: &ExprLike) {
        Visit::visit_expr_like(&mut **self, i);
    }

    fn visit_expr_map(&mut self, i: &ExprMap) {
        Visit::visit_expr_map(&mut **self, i);
    }

    fn visit_expr_match(&mut self, i: &ExprMatch) {
        Visit::visit_expr_match(&mut **self, i);
    }

    fn visit_expr_not(&mut self, i: &ExprNot) {
        Visit::visit_expr_not(&mut **self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr) {
        Visit::visit_expr_or(&mut **self, i);
    }

    fn visit_expr_list(&mut self, i: &ExprList) {
        Visit::visit_expr_list(&mut **self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord) {
        Visit::visit_expr_record(&mut **self, i);
    }

    fn visit_expr_reference(&mut self, i: &ExprReference) {
        Visit::visit_expr_reference(&mut **self, i);
    }

    fn visit_expr_set(&mut self, i: &ExprSet) {
        Visit::visit_expr_set(&mut **self, i);
    }

    fn visit_expr_set_op(&mut self, i: &ExprSetOp) {
        Visit::visit_expr_set_op(&mut **self, i);
    }

    fn visit_expr_stmt(&mut self, i: &ExprStmt) {
        Visit::visit_expr_stmt(&mut **self, i);
    }

    fn visit_filter(&mut self, i: &Filter) {
        Visit::visit_filter(&mut **self, i);
    }

    fn visit_condition(&mut self, i: &Condition) {
        Visit::visit_condition(&mut **self, i);
    }

    fn visit_expr_project(&mut self, i: &ExprProject) {
        Visit::visit_expr_project(&mut **self, i);
    }

    fn visit_insert_target(&mut self, i: &InsertTarget) {
        Visit::visit_insert_target(&mut **self, i);
    }

    fn visit_join(&mut self, i: &Join) {
        Visit::visit_join(&mut **self, i);
    }

    fn visit_limit(&mut self, i: &Limit) {
        Visit::visit_limit(&mut **self, i);
    }

    fn visit_limit_cursor(&mut self, i: &LimitCursor) {
        Visit::visit_limit_cursor(&mut **self, i);
    }

    fn visit_limit_offset(&mut self, i: &LimitOffset) {
        Visit::visit_limit_offset(&mut **self, i);
    }

    fn visit_order_by(&mut self, i: &OrderBy) {
        Visit::visit_order_by(&mut **self, i);
    }

    fn visit_order_by_expr(&mut self, i: &OrderByExpr) {
        Visit::visit_order_by_expr(&mut **self, i);
    }

    fn visit_path(&mut self, i: &Path) {
        Visit::visit_path(&mut **self, i);
    }

    fn visit_projection(&mut self, i: &Projection) {
        Visit::visit_projection(&mut **self, i);
    }

    fn visit_returning(&mut self, i: &Returning) {
        Visit::visit_returning(&mut **self, i);
    }

    fn visit_source(&mut self, i: &Source) {
        Visit::visit_source(&mut **self, i);
    }

    fn visit_source_model(&mut self, i: &SourceModel) {
        Visit::visit_source_model(&mut **self, i);
    }

    fn visit_source_table(&mut self, i: &SourceTable) {
        Visit::visit_source_table(&mut **self, i);
    }

    fn visit_source_table_id(&mut self, i: &SourceTableId) {
        Visit::visit_source_table_id(&mut **self, i);
    }

    fn visit_stmt(&mut self, i: &Statement) {
        Visit::visit_stmt(&mut **self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete) {
        Visit::visit_stmt_delete(&mut **self, i);
    }

    fn visit_stmt_insert(&mut self, i: &Insert) {
        Visit::visit_stmt_insert(&mut **self, i);
    }

    fn visit_stmt_query(&mut self, i: &Query) {
        Visit::visit_stmt_query(&mut **self, i);
    }

    fn visit_stmt_select(&mut self, i: &Select) {
        Visit::visit_stmt_select(&mut **self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update) {
        Visit::visit_stmt_update(&mut **self, i);
    }

    fn visit_table_derived(&mut self, i: &TableDerived) {
        Visit::visit_table_derived(&mut **self, i);
    }

    fn visit_table_ref(&mut self, i: &TableRef) {
        Visit::visit_table_ref(&mut **self, i);
    }

    fn visit_table_factor(&mut self, i: &TableFactor) {
        Visit::visit_table_factor(&mut **self, i);
    }

    fn visit_table_with_joins(&mut self, i: &TableWithJoins) {
        Visit::visit_table_with_joins(&mut **self, i);
    }

    fn visit_type(&mut self, i: &Type) {
        Visit::visit_type(&mut **self, i);
    }

    fn visit_update_target(&mut self, i: &UpdateTarget) {
        Visit::visit_update_target(&mut **self, i);
    }

    fn visit_value(&mut self, i: &Value) {
        Visit::visit_value(&mut **self, i);
    }

    fn visit_value_record(&mut self, i: &ValueRecord) {
        Visit::visit_value_record(&mut **self, i);
    }

    fn visit_values(&mut self, i: &Values) {
        Visit::visit_values(&mut **self, i);
    }

    fn visit_with(&mut self, i: &With) {
        Visit::visit_with(&mut **self, i);
    }
}

/// Default traversal for [`Assignment`] nodes. Visits the assignment's expression(s).
pub fn visit_assignment<V>(v: &mut V, node: &Assignment)
where
    V: Visit + ?Sized,
{
    match node {
        Assignment::Set(expr) | Assignment::Insert(expr) | Assignment::Remove(expr) => {
            v.visit_expr(expr);
        }
        Assignment::Batch(entries) => {
            for entry in entries {
                visit_assignment(v, entry);
            }
        }
    }
}

/// Default traversal for [`Assignments`] nodes. Visits each assignment in the collection.
pub fn visit_assignments<V>(v: &mut V, node: &Assignments)
where
    V: Visit + ?Sized,
{
    for (_, assignment) in node.iter() {
        v.visit_assignment(assignment);
    }
}

/// Default traversal for [`Association`] nodes. Visits the association's source query.
pub fn visit_association<V>(v: &mut V, node: &Association)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.source);
}

/// Default traversal for [`Cte`] nodes. Visits the CTE's query.
pub fn visit_cte<V>(v: &mut V, node: &Cte)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.query);
}

/// Default traversal for [`Expr`] nodes. Dispatches to the appropriate expression visitor based on variant.
pub fn visit_expr<V>(v: &mut V, node: &Expr)
where
    V: Visit + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and(expr),
        Expr::Any(expr) => v.visit_expr_any(expr),
        Expr::Arg(expr) => v.visit_expr_arg(expr),
        Expr::BeginsWith(expr) => v.visit_expr_begins_with(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op(expr),
        Expr::Cast(expr) => v.visit_expr_cast(expr),
        Expr::Default => v.visit_expr_default(),
        Expr::Error(expr) => v.visit_expr_error(expr),
        Expr::Exists(expr) => v.visit_expr_exists(expr),
        Expr::Func(expr) => v.visit_expr_func(expr),
        Expr::Ident(_) => {}
        Expr::InList(expr) => v.visit_expr_in_list(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery(expr),
        Expr::IsNull(expr) => v.visit_expr_is_null(expr),
        Expr::IsVariant(expr) => v.visit_expr_is_variant(expr),
        Expr::Let(expr) => v.visit_expr_let(expr),
        Expr::Like(expr) => v.visit_expr_like(expr),
        Expr::Map(expr) => v.visit_expr_map(expr),
        Expr::Match(expr) => v.visit_expr_match(expr),
        Expr::Not(expr) => v.visit_expr_not(expr),
        Expr::Or(expr) => v.visit_expr_or(expr),
        Expr::Project(expr) => v.visit_expr_project(expr),
        Expr::Record(expr) => v.visit_expr_record(expr),
        Expr::Reference(expr) => v.visit_expr_reference(expr),
        Expr::List(expr) => v.visit_expr_list(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt(expr),
        Expr::Value(expr) => v.visit_value(expr),
    }
}

/// Default traversal for [`ExprAnd`] nodes. Visits each operand expression.
pub fn visit_expr_and<V>(v: &mut V, node: &ExprAnd)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`ExprAny`] nodes. Visits the inner expression.
pub fn visit_expr_any<V>(v: &mut V, node: &ExprAny)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

/// Default traversal for [`ExprArg`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_arg<V>(v: &mut V, node: &ExprArg)
where
    V: Visit + ?Sized,
{
}

/// Default traversal for [`ExprBeginsWith`] nodes. Visits the attribute expression and prefix.
pub fn visit_expr_begins_with<V>(v: &mut V, node: &ExprBeginsWith)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.prefix);
}

/// Default traversal for [`ExprBinaryOp`] nodes. Visits left and right operands.
pub fn visit_expr_binary_op<V>(v: &mut V, node: &ExprBinaryOp)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.lhs);
    v.visit_expr(&node.rhs);
}

/// Default traversal for [`ExprCast`] nodes. Visits the inner expression and target type.
pub fn visit_expr_cast<V>(v: &mut V, node: &ExprCast)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_type(&node.ty);
}

/// Default traversal for [`ExprColumn`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_column<V>(v: &mut V, node: &ExprColumn)
where
    V: Visit + ?Sized,
{
}

/// Default traversal for [`Expr::Default`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_default<V>(v: &mut V)
where
    V: Visit + ?Sized,
{
}

/// Default traversal for [`ExprError`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_error<V>(v: &mut V, node: &ExprError)
where
    V: Visit + ?Sized,
{
    // ExprError has no child expressions to visit
}

/// Default traversal for [`ExprExists`] nodes. Visits the subquery.
pub fn visit_expr_exists<V>(v: &mut V, node: &ExprExists)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.subquery);
}

/// Default traversal for [`ExprFunc`] nodes. Dispatches to the specific function visitor.
pub fn visit_expr_func<V>(v: &mut V, node: &ExprFunc)
where
    V: Visit + ?Sized,
{
    match node {
        ExprFunc::Count(func) => v.visit_expr_func_count(func),
        ExprFunc::LastInsertId(func) => v.visit_expr_func_last_insert_id(func),
    }
}

/// Default traversal for [`FuncCount`] nodes. Visits the optional argument and filter expressions.
pub fn visit_expr_func_count<V>(v: &mut V, node: &FuncCount)
where
    V: Visit + ?Sized,
{
    if let Some(expr) = &node.arg {
        v.visit_expr(expr);
    }

    if let Some(expr) = &node.filter {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`FuncLastInsertId`] nodes. This is a leaf node with no children to visit.
pub fn visit_expr_func_last_insert_id<V>(_v: &mut V, _node: &FuncLastInsertId)
where
    V: Visit + ?Sized,
{
    // FuncLastInsertId has no fields to visit
}

/// Default traversal for [`ExprInList`] nodes. Visits the expression and list.
pub fn visit_expr_in_list<V>(v: &mut V, node: &ExprInList)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.list);
}

/// Default traversal for [`ExprInSubquery`] nodes. Visits the expression and subquery.
pub fn visit_expr_in_subquery<V>(v: &mut V, node: &ExprInSubquery)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_stmt_query(&node.query);
}

/// Default traversal for [`ExprIsNull`] nodes. Visits the inner expression.
pub fn visit_expr_is_null<V>(v: &mut V, node: &ExprIsNull)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

/// Default traversal for [`ExprIsVariant`] nodes. Visits the inner expression.
pub fn visit_expr_is_variant<V>(v: &mut V, node: &ExprIsVariant)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

/// Default traversal for [`ExprLet`] nodes. Visits bindings and body.
pub fn visit_expr_let<V>(v: &mut V, node: &ExprLet)
where
    V: Visit + ?Sized,
{
    for binding in &node.bindings {
        v.visit_expr(binding);
    }
    v.visit_expr(&node.body);
}

/// Default traversal for [`ExprLike`] nodes. Visits the attribute expression and pattern.
pub fn visit_expr_like<V>(v: &mut V, node: &ExprLike)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

/// Default traversal for [`ExprMap`] nodes. Visits base and map expressions.
pub fn visit_expr_map<V>(v: &mut V, node: &ExprMap)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_expr(&node.map);
}

/// Default traversal for [`ExprMatch`] nodes. Visits subject, arms, and else expression.
pub fn visit_expr_match<V>(v: &mut V, node: &ExprMatch)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.subject);
    for arm in &node.arms {
        v.visit_expr(&arm.expr);
    }
    v.visit_expr(&node.else_expr);
}

/// Default traversal for [`ExprNot`] nodes. Visits the inner expression.
pub fn visit_expr_not<V>(v: &mut V, node: &ExprNot)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

/// Default traversal for [`ExprOr`] nodes. Visits each operand expression.
pub fn visit_expr_or<V>(v: &mut V, node: &ExprOr)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`ExprList`] nodes. Visits each item expression.
pub fn visit_expr_list<V>(v: &mut V, node: &ExprList)
where
    V: Visit + ?Sized,
{
    for expr in &node.items {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`ExprRecord`] nodes. Visits each field expression.
pub fn visit_expr_record<V>(v: &mut V, node: &ExprRecord)
where
    V: Visit + ?Sized,
{
    for expr in &**node {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`ExprReference`] nodes. Dispatches based on reference kind.
pub fn visit_expr_reference<V>(v: &mut V, node: &ExprReference)
where
    V: Visit + ?Sized,
{
    match node {
        ExprReference::Model { .. } => {}
        ExprReference::Field { .. } => {}
        ExprReference::Column(expr_column) => v.visit_expr_column(expr_column),
    }
}

/// Default traversal for [`ExprSet`] nodes. Dispatches to the appropriate set expression visitor.
pub fn visit_expr_set<V>(v: &mut V, node: &ExprSet)
where
    V: Visit + ?Sized,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op(expr),
        ExprSet::Update(expr) => v.visit_stmt_update(expr),
        ExprSet::Values(expr) => v.visit_values(expr),
        ExprSet::Insert(expr) => v.visit_stmt_insert(expr),
    }
}

/// Default traversal for [`ExprSetOp`] nodes. Visits each operand.
pub fn visit_expr_set_op<V>(v: &mut V, node: &ExprSetOp)
where
    V: Visit + ?Sized,
{
    for operand in &node.operands {
        v.visit_expr_set(operand);
    }
}

/// Default traversal for [`ExprStmt`] nodes. Visits the inner statement.
pub fn visit_expr_stmt<V>(v: &mut V, node: &ExprStmt)
where
    V: Visit + ?Sized,
{
    v.visit_stmt(&node.stmt);
}

/// Default traversal for [`ExprProject`] nodes. Visits the base expression and projection.
pub fn visit_expr_project<V>(v: &mut V, node: &ExprProject)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_projection(&node.projection);
}

/// Default traversal for [`Filter`] nodes. Visits the optional filter expression.
pub fn visit_filter<V>(v: &mut V, node: &Filter)
where
    V: Visit + ?Sized,
{
    if let Some(expr) = &node.expr {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`Condition`] nodes. Visits the optional condition expression.
pub fn visit_condition<V>(v: &mut V, node: &Condition)
where
    V: Visit + ?Sized,
{
    if let Some(expr) = &node.expr {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`InsertTarget`] nodes. Visits the scope query if present.
pub fn visit_insert_target<V>(v: &mut V, node: &InsertTarget)
where
    V: Visit + ?Sized,
{
    if let InsertTarget::Scope(stmt) = node {
        v.visit_stmt_query(stmt);
    }
}

/// Default traversal for [`Join`] nodes. Visits the table and join constraint.
pub fn visit_join<V>(v: &mut V, node: &Join)
where
    V: Visit + ?Sized,
{
    v.visit_source_table_id(&node.table);
    match &node.constraint {
        JoinOp::Left(expr) => v.visit_expr(expr),
    }
}

/// Default traversal for [`Limit`] nodes.
pub fn visit_limit<V>(v: &mut V, node: &Limit)
where
    V: Visit + ?Sized,
{
    match node {
        Limit::Cursor(cursor) => v.visit_limit_cursor(cursor),
        Limit::Offset(offset) => v.visit_limit_offset(offset),
    }
}

/// Default traversal for [`LimitCursor`] nodes.
pub fn visit_limit_cursor<V>(v: &mut V, node: &LimitCursor)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.page_size);
    if let Some(after) = &node.after {
        v.visit_expr(after);
    }
}

/// Default traversal for [`LimitOffset`] nodes.
pub fn visit_limit_offset<V>(v: &mut V, node: &LimitOffset)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.limit);
    if let Some(offset) = &node.offset {
        v.visit_expr(offset);
    }
}

/// Default traversal for [`OrderBy`] nodes. Visits each ordering expression.
pub fn visit_order_by<V>(v: &mut V, node: &OrderBy)
where
    V: Visit + ?Sized,
{
    for expr in &node.exprs {
        v.visit_order_by_expr(expr);
    }
}

/// Default traversal for [`OrderByExpr`] nodes. Visits the ordering expression.
pub fn visit_order_by_expr<V>(v: &mut V, node: &OrderByExpr)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

/// Default traversal for [`Path`] nodes. Visits the path's projection.
pub fn visit_path<V>(v: &mut V, node: &Path)
where
    V: Visit + ?Sized,
{
    v.visit_projection(&node.projection);
}

/// Default traversal for [`Projection`] nodes. This is a leaf node with no children to visit.
pub fn visit_projection<V>(v: &mut V, node: &Projection)
where
    V: Visit + ?Sized,
{
}

/// Default traversal for [`Returning`] nodes. Visits included paths or expressions based on variant.
pub fn visit_returning<V>(v: &mut V, node: &Returning)
where
    V: Visit + ?Sized,
{
    match node {
        Returning::Model { include } => {
            for path in include {
                v.visit_path(path);
            }
        }
        Returning::Changed => {}
        Returning::Expr(expr) => v.visit_expr(expr),
        Returning::Value(expr) => v.visit_expr(expr),
    }
}

/// Default traversal for [`Source`] nodes. Dispatches to model or table source visitor.
pub fn visit_source<V>(v: &mut V, node: &Source)
where
    V: Visit + ?Sized,
{
    match node {
        Source::Model(source_model) => v.visit_source_model(source_model),
        Source::Table(source_table) => v.visit_source_table(source_table),
    }
}

/// Default traversal for [`SourceModel`] nodes. Visits the optional association.
pub fn visit_source_model<V>(v: &mut V, node: &SourceModel)
where
    V: Visit + ?Sized,
{
    if let Some(association) = &node.via {
        v.visit_association(association);
    }
}

/// Default traversal for [`SourceTable`] nodes. Visits table references and FROM clauses.
pub fn visit_source_table<V>(v: &mut V, node: &SourceTable)
where
    V: Visit + ?Sized,
{
    for table_ref in &node.tables {
        v.visit_table_ref(table_ref);
    }
    for table_with_joins in &node.from {
        v.visit_table_with_joins(table_with_joins);
    }
}

/// Default traversal for [`SourceTableId`] nodes. This is a leaf node with no children to visit.
pub fn visit_source_table_id<V>(v: &mut V, node: &SourceTableId)
where
    V: Visit + ?Sized,
{
    // SourceTableId is just an index, nothing to visit
}

/// Default traversal for [`TableFactor`] nodes. Dispatches based on factor type.
pub fn visit_table_factor<V>(v: &mut V, node: &TableFactor)
where
    V: Visit + ?Sized,
{
    match node {
        TableFactor::Table(table_id) => v.visit_source_table_id(table_id),
    }
}

/// Default traversal for [`Statement`] nodes. Dispatches to the appropriate statement visitor.
pub fn visit_stmt<V>(v: &mut V, node: &Statement)
where
    V: Visit + ?Sized,
{
    match node {
        Statement::Delete(stmt) => v.visit_stmt_delete(stmt),
        Statement::Insert(stmt) => v.visit_stmt_insert(stmt),
        Statement::Query(stmt) => v.visit_stmt_query(stmt),
        Statement::Update(stmt) => v.visit_stmt_update(stmt),
    }
}

/// Default traversal for [`Delete`] nodes. Visits source, filter, and optional returning.
pub fn visit_stmt_delete<V>(v: &mut V, node: &Delete)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.from);
    v.visit_filter(&node.filter);
    v.visit_condition(&node.condition);

    if let Some(returning) = &node.returning {
        v.visit_returning(returning);
    }
}

/// Default traversal for [`Insert`] nodes. Visits target, source query, and optional returning.
pub fn visit_stmt_insert<V>(v: &mut V, node: &Insert)
where
    V: Visit + ?Sized,
{
    if let InsertTarget::Scope(scope) = &node.target {
        v.visit_stmt_query(scope);
    }
    v.visit_stmt_query(&node.source);

    if let Some(returning) = &node.returning {
        v.visit_returning(returning);
    }
}

/// Default traversal for [`Query`] nodes. Visits optional WITH, body, order by, and limit.
pub fn visit_stmt_query<V>(v: &mut V, node: &Query)
where
    V: Visit + ?Sized,
{
    if let Some(with) = &node.with {
        v.visit_with(with);
    }

    v.visit_expr_set(&node.body);

    if let Some(order_by) = &node.order_by {
        v.visit_order_by(order_by);
    }

    if let Some(limit) = &node.limit {
        v.visit_limit(limit);
    }
}

/// Default traversal for [`Select`] nodes. Visits source, filter, and returning.
pub fn visit_stmt_select<V>(v: &mut V, node: &Select)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.source);
    v.visit_filter(&node.filter);
    v.visit_returning(&node.returning);
}

/// Default traversal for [`Update`] nodes. Visits target, assignments, filter, and condition.
pub fn visit_stmt_update<V>(v: &mut V, node: &Update)
where
    V: Visit + ?Sized,
{
    v.visit_update_target(&node.target);
    v.visit_assignments(&node.assignments);
    v.visit_filter(&node.filter);
    v.visit_condition(&node.condition);
}

/// Default traversal for [`TableDerived`] nodes. Visits the subquery.
pub fn visit_table_derived<V>(v: &mut V, node: &TableDerived)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.subquery);
}

/// Default traversal for [`TableRef`] nodes. Dispatches based on reference kind.
pub fn visit_table_ref<V>(v: &mut V, node: &TableRef)
where
    V: Visit + ?Sized,
{
    match node {
        TableRef::Cte { .. } => {}
        TableRef::Derived(table_derived) => v.visit_table_derived(table_derived),
        TableRef::Table(_) => {}
        TableRef::Arg(expr_arg) => v.visit_expr_arg(expr_arg),
    }
}

/// Default traversal for [`TableWithJoins`] nodes. Visits the relation and each join.
pub fn visit_table_with_joins<V>(v: &mut V, node: &TableWithJoins)
where
    V: Visit + ?Sized,
{
    v.visit_table_factor(&node.relation);
    for join in &node.joins {
        v.visit_join(join);
    }
}

/// Default traversal for [`Type`] nodes. This is a leaf node with no children to visit.
pub fn visit_type<V>(v: &mut V, node: &Type)
where
    V: Visit + ?Sized,
{
    // Type is just type information, no traversal needed
}

/// Default traversal for [`UpdateTarget`] nodes. Visits the query if target is a query.
pub fn visit_update_target<V>(v: &mut V, node: &UpdateTarget)
where
    V: Visit + ?Sized,
{
    if let UpdateTarget::Query(query) = node {
        v.visit_stmt_query(query)
    }
}

/// Default traversal for [`Value`] nodes. Visits inner record if value is a record.
pub fn visit_value<V>(v: &mut V, node: &Value)
where
    V: Visit + ?Sized,
{
    if let Value::Record(node) = node {
        v.visit_value_record(node)
    }
}

/// Default traversal for [`ValueRecord`] nodes. Visits each value in the record.
pub fn visit_value_record<V>(v: &mut V, node: &ValueRecord)
where
    V: Visit + ?Sized,
{
    for value in node.iter() {
        v.visit_value(value);
    }
}

/// Default traversal for [`Values`] nodes. Visits each row expression.
pub fn visit_values<V>(v: &mut V, node: &Values)
where
    V: Visit + ?Sized,
{
    for expr in &node.rows {
        v.visit_expr(expr);
    }
}

/// Default traversal for [`With`] nodes. Visits each CTE.
pub fn visit_with<V>(v: &mut V, node: &With)
where
    V: Visit + ?Sized,
{
    for cte in &node.ctes {
        v.visit_cte(cte);
    }
}

/// Calls `f` for every [`Expr`] node reachable from `node`, in post-order.
///
/// This is a convenience wrapper that constructs a [`Visit`] implementation
/// internally and walks the full AST rooted at `node`.
pub fn for_each_expr<F>(node: &impl Node, f: F)
where
    F: FnMut(&Expr),
{
    struct ForEach<F> {
        f: F,
    }

    impl<F> Visit for ForEach<F>
    where
        F: FnMut(&Expr),
    {
        fn visit_expr(&mut self, node: &Expr) {
            visit_expr(self, node);
            (self.f)(node);
        }
    }

    node.visit(ForEach { f });
}
