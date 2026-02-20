#![allow(unused_variables)]

use super::{
    Assignment, Assignments, Association, Condition, Cte, Delete, Expr, ExprAnd, ExprAny, ExprArg,
    ExprBeginsWith, ExprBinaryOp, ExprCast, ExprColumn, ExprConcat, ExprExists, ExprFunc,
    ExprInList, ExprInSubquery, ExprIsNull, ExprKey, ExprLike, ExprList, ExprMap, ExprNot, ExprOr,
    ExprPattern, ExprProject, ExprRecord, ExprReference, ExprSet, ExprSetOp, ExprStmt, ExprTy,
    Filter, FuncCount, FuncLastInsertId, Insert, InsertTarget, Join, JoinOp, Limit, Node, Offset,
    OrderBy, OrderByExpr, Path, Projection, Query, Returning, Select, Source, SourceModel,
    SourceTable, SourceTableId, Statement, TableDerived, TableFactor, TableRef, TableWithJoins,
    Type, Update, UpdateTarget, Value, ValueRecord, Values, With,
};

pub trait VisitMut {
    fn visit_mut<N: Node>(&mut self, i: &mut N)
    where
        Self: Sized,
    {
        i.visit_mut(self);
    }

    fn visit_assignment_mut(&mut self, i: &mut Assignment) {
        visit_assignment_mut(self, i);
    }

    fn visit_assignments_mut(&mut self, i: &mut Assignments) {
        visit_assignments_mut(self, i);
    }

    fn visit_association_mut(&mut self, i: &mut Association) {
        visit_association_mut(self, i);
    }

    fn visit_cte_mut(&mut self, i: &mut Cte) {
        visit_cte_mut(self, i);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        visit_expr_mut(self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd) {
        visit_expr_and_mut(self, i);
    }

    fn visit_expr_any_mut(&mut self, i: &mut ExprAny) {
        visit_expr_any_mut(self, i);
    }

    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        visit_expr_arg_mut(self, i);
    }

    fn visit_expr_begins_with_mut(&mut self, i: &mut ExprBeginsWith) {
        visit_expr_begins_with_mut(self, i);
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp) {
        visit_expr_binary_op_mut(self, i);
    }

    fn visit_expr_cast_mut(&mut self, i: &mut ExprCast) {
        visit_expr_cast_mut(self, i);
    }

    fn visit_expr_column_mut(&mut self, i: &mut ExprColumn) {
        visit_expr_column_mut(self, i);
    }

    fn visit_expr_concat_mut(&mut self, i: &mut ExprConcat) {
        visit_expr_concat_mut(self, i);
    }

    fn visit_expr_default_mut(&mut self) {
        visit_expr_default_mut(self);
    }

    fn visit_expr_exists_mut(&mut self, i: &mut ExprExists) {
        visit_expr_exists_mut(self, i);
    }

    fn visit_expr_func_mut(&mut self, i: &mut ExprFunc) {
        visit_expr_func_mut(self, i);
    }

    fn visit_expr_func_count_mut(&mut self, i: &mut FuncCount) {
        visit_expr_func_count_mut(self, i);
    }

    fn visit_expr_func_last_insert_id_mut(&mut self, i: &mut FuncLastInsertId) {
        visit_expr_func_last_insert_id_mut(self, i);
    }

    fn visit_expr_in_list_mut(&mut self, i: &mut ExprInList) {
        visit_expr_in_list_mut(self, i);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery) {
        visit_expr_in_subquery_mut(self, i);
    }

    fn visit_expr_is_null_mut(&mut self, i: &mut ExprIsNull) {
        visit_expr_is_null_mut(self, i);
    }

    fn visit_expr_like_mut(&mut self, i: &mut ExprLike) {
        visit_expr_like_mut(self, i);
    }

    fn visit_expr_key_mut(&mut self, i: &mut ExprKey) {
        visit_expr_key_mut(self, i);
    }

    fn visit_expr_map_mut(&mut self, i: &mut ExprMap) {
        visit_expr_map_mut(self, i);
    }

    fn visit_expr_not_mut(&mut self, i: &mut ExprNot) {
        visit_expr_not_mut(self, i);
    }

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr) {
        visit_expr_or_mut(self, i);
    }

    fn visit_expr_list_mut(&mut self, i: &mut ExprList) {
        visit_expr_list_mut(self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord) {
        visit_expr_record_mut(self, i);
    }

    fn visit_expr_reference_mut(&mut self, i: &mut ExprReference) {
        visit_expr_reference_mut(self, i);
    }

    fn visit_expr_set_mut(&mut self, i: &mut ExprSet) {
        visit_expr_set_mut(self, i);
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp) {
        visit_expr_set_op_mut(self, i);
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt) {
        visit_expr_stmt_mut(self, i);
    }

    fn visit_expr_ty_mut(&mut self, i: &mut ExprTy) {
        visit_expr_ty_mut(self, i);
    }

    fn visit_expr_pattern_mut(&mut self, i: &mut ExprPattern) {
        visit_expr_pattern_mut(self, i);
    }

    fn visit_filter_mut(&mut self, i: &mut Filter) {
        visit_filter_mut(self, i);
    }

    fn visit_condition_mut(&mut self, i: &mut Condition) {
        visit_condition_mut(self, i);
    }

    fn visit_expr_project_mut(&mut self, i: &mut ExprProject) {
        visit_expr_project_mut(self, i);
    }

    fn visit_insert_target_mut(&mut self, i: &mut InsertTarget) {
        visit_insert_target_mut(self, i);
    }

    fn visit_join_mut(&mut self, i: &mut Join) {
        visit_join_mut(self, i);
    }

    fn visit_limit_mut(&mut self, i: &mut Limit) {
        visit_limit_mut(self, i);
    }

    fn visit_offset_mut(&mut self, i: &mut Offset) {
        visit_offset_mut(self, i);
    }

    fn visit_order_by_mut(&mut self, i: &mut OrderBy) {
        visit_order_by_mut(self, i);
    }

    fn visit_order_by_expr_mut(&mut self, i: &mut OrderByExpr) {
        visit_order_by_expr_mut(self, i);
    }

    fn visit_path_mut(&mut self, i: &mut Path) {
        visit_path_mut(self, i);
    }

    fn visit_projection_mut(&mut self, i: &mut Projection) {
        visit_projection_mut(self, i);
    }

    fn visit_returning_mut(&mut self, i: &mut Returning) {
        visit_returning_mut(self, i);
    }

    fn visit_source_mut(&mut self, i: &mut Source) {
        visit_source_mut(self, i);
    }

    fn visit_source_model_mut(&mut self, i: &mut SourceModel) {
        visit_source_model_mut(self, i);
    }

    fn visit_source_table_mut(&mut self, i: &mut SourceTable) {
        visit_source_table_mut(self, i);
    }

    fn visit_source_table_id_mut(&mut self, i: &mut SourceTableId) {
        visit_source_table_id_mut(self, i);
    }

    fn visit_stmt_mut(&mut self, i: &mut Statement) {
        visit_stmt_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete) {
        visit_stmt_delete_mut(self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert) {
        visit_stmt_insert_mut(self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query) {
        visit_stmt_query_mut(self, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut Select) {
        visit_stmt_select_mut(self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update) {
        visit_stmt_update_mut(self, i);
    }

    fn visit_table_derived_mut(&mut self, i: &mut TableDerived) {
        visit_table_derived_mut(self, i);
    }

    fn visit_table_ref_mut(&mut self, i: &mut TableRef) {
        visit_table_ref_mut(self, i);
    }

    fn visit_table_factor_mut(&mut self, i: &mut TableFactor) {
        visit_table_factor_mut(self, i);
    }

    fn visit_table_with_joins_mut(&mut self, i: &mut TableWithJoins) {
        visit_table_with_joins_mut(self, i);
    }

    fn visit_type_mut(&mut self, i: &mut Type) {
        visit_type_mut(self, i);
    }

    fn visit_update_target_mut(&mut self, i: &mut UpdateTarget) {
        visit_update_target_mut(self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value) {
        visit_value_mut(self, i);
    }

    fn visit_value_record(&mut self, i: &mut ValueRecord) {
        visit_value_record(self, i);
    }

    fn visit_values_mut(&mut self, i: &mut Values) {
        visit_values_mut(self, i);
    }

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

    fn visit_expr_concat_mut(&mut self, i: &mut ExprConcat) {
        VisitMut::visit_expr_concat_mut(&mut **self, i);
    }

    fn visit_expr_default_mut(&mut self) {
        VisitMut::visit_expr_default_mut(&mut **self);
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

    fn visit_expr_like_mut(&mut self, i: &mut ExprLike) {
        VisitMut::visit_expr_like_mut(&mut **self, i);
    }

    fn visit_expr_key_mut(&mut self, i: &mut ExprKey) {
        VisitMut::visit_expr_key_mut(&mut **self, i);
    }

    fn visit_expr_map_mut(&mut self, i: &mut ExprMap) {
        VisitMut::visit_expr_map_mut(&mut **self, i);
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

    fn visit_expr_ty_mut(&mut self, i: &mut ExprTy) {
        VisitMut::visit_expr_ty_mut(&mut **self, i);
    }

    fn visit_expr_pattern_mut(&mut self, i: &mut ExprPattern) {
        VisitMut::visit_expr_pattern_mut(&mut **self, i);
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

    fn visit_offset_mut(&mut self, i: &mut Offset) {
        VisitMut::visit_offset_mut(&mut **self, i);
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

pub fn visit_assignment_mut<V>(v: &mut V, node: &mut Assignment)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_assignments_mut<V>(v: &mut V, node: &mut Assignments)
where
    V: VisitMut + ?Sized,
{
    for (_, assignment) in node.iter_mut() {
        v.visit_assignment_mut(assignment);
    }
}

pub fn visit_association_mut<V>(v: &mut V, node: &mut Association)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.source);
}

pub fn visit_cte_mut<V>(v: &mut V, node: &mut Cte)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.query);
}

pub fn visit_expr_mut<V>(v: &mut V, node: &mut Expr)
where
    V: VisitMut + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and_mut(expr),
        Expr::Any(expr) => v.visit_expr_any_mut(expr),
        Expr::Arg(expr) => v.visit_expr_arg_mut(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op_mut(expr),
        Expr::Cast(expr) => v.visit_expr_cast_mut(expr),
        Expr::Concat(expr) => v.visit_expr_concat_mut(expr),
        Expr::Default => v.visit_expr_default_mut(),
        Expr::Exists(expr) => v.visit_expr_exists_mut(expr),
        Expr::Func(expr) => v.visit_expr_func_mut(expr),
        Expr::InList(expr) => v.visit_expr_in_list_mut(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery_mut(expr),
        Expr::IsNull(expr) => v.visit_expr_is_null_mut(expr),
        Expr::Key(expr) => v.visit_expr_key_mut(expr),
        Expr::Map(expr) => v.visit_expr_map_mut(expr),
        Expr::Not(expr) => v.visit_expr_not_mut(expr),
        Expr::Or(expr) => v.visit_expr_or_mut(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern_mut(expr),
        Expr::Project(expr) => v.visit_expr_project_mut(expr),
        Expr::Record(expr) => v.visit_expr_record_mut(expr),
        Expr::Reference(expr) => v.visit_expr_reference_mut(expr),
        Expr::List(expr) => v.visit_expr_list_mut(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt_mut(expr),
        Expr::Type(expr) => v.visit_expr_ty_mut(expr),
        Expr::Value(expr) => v.visit_value_mut(expr),
        // HAX
        Expr::ConcatStr(expr) => {
            for expr in &mut expr.exprs {
                v.visit_expr_mut(expr);
            }
        }
    }
}

pub fn visit_expr_and_mut<V>(v: &mut V, node: &mut ExprAnd)
where
    V: VisitMut + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_any_mut<V>(v: &mut V, node: &mut ExprAny)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_expr_arg_mut<V>(v: &mut V, node: &mut ExprArg)
where
    V: VisitMut + ?Sized,
{
}

pub fn visit_expr_begins_with_mut<V>(v: &mut V, node: &mut ExprBeginsWith)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_binary_op_mut<V>(v: &mut V, node: &mut ExprBinaryOp)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.lhs);
    v.visit_expr_mut(&mut node.rhs);
}

pub fn visit_expr_cast_mut<V>(v: &mut V, node: &mut ExprCast)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_type_mut(&mut node.ty);
}

pub fn visit_expr_column_mut<V>(v: &mut V, node: &mut ExprColumn)
where
    V: VisitMut + ?Sized,
{
}

pub fn visit_expr_concat_mut<V>(v: &mut V, node: &mut ExprConcat)
where
    V: VisitMut + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_default_mut<V>(v: &mut V)
where
    V: VisitMut + ?Sized,
{
}

pub fn visit_expr_exists_mut<V>(v: &mut V, node: &mut ExprExists)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.subquery);
}

pub fn visit_expr_func_mut<V>(v: &mut V, node: &mut ExprFunc)
where
    V: VisitMut + ?Sized,
{
    match node {
        ExprFunc::Count(func) => v.visit_expr_func_count_mut(func),
        ExprFunc::LastInsertId(func) => v.visit_expr_func_last_insert_id_mut(func),
    }
}

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

pub fn visit_expr_func_last_insert_id_mut<V>(_v: &mut V, _node: &mut FuncLastInsertId)
where
    V: VisitMut + ?Sized,
{
    // FuncLastInsertId has no fields to visit
}

pub fn visit_expr_in_list_mut<V>(v: &mut V, node: &mut ExprInList)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.list);
}

pub fn visit_expr_in_subquery_mut<V>(v: &mut V, node: &mut ExprInSubquery)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_stmt_query_mut(&mut node.query);
}

pub fn visit_expr_is_null_mut<V>(v: &mut V, node: &mut ExprIsNull)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_expr_like_mut<V>(v: &mut V, node: &mut ExprLike)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_key_mut<V>(v: &mut V, node: &mut ExprKey)
where
    V: VisitMut + ?Sized,
{
}

pub fn visit_expr_map_mut<V>(v: &mut V, node: &mut ExprMap)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_expr_mut(&mut node.map);
}

pub fn visit_expr_not_mut<V>(v: &mut V, node: &mut ExprNot)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_expr_or_mut<V>(v: &mut V, node: &mut ExprOr)
where
    V: VisitMut + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_list_mut<V>(v: &mut V, node: &mut ExprList)
where
    V: VisitMut + ?Sized,
{
    for e in &mut node.items {
        v.visit_expr_mut(e);
    }
}

pub fn visit_expr_record_mut<V>(v: &mut V, node: &mut ExprRecord)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut **node {
        v.visit_expr_mut(expr);
    }
}

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

pub fn visit_expr_set_op_mut<V>(v: &mut V, node: &mut ExprSetOp)
where
    V: VisitMut + ?Sized,
{
    for operand in &mut node.operands {
        v.visit_expr_set_mut(operand);
    }
}

pub fn visit_expr_stmt_mut<V>(v: &mut V, node: &mut ExprStmt)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_mut(&mut node.stmt);
}

pub fn visit_expr_ty_mut<V>(v: &mut V, node: &mut ExprTy)
where
    V: VisitMut + ?Sized,
{
    v.visit_type_mut(&mut node.ty);
}

pub fn visit_expr_pattern_mut<V>(v: &mut V, node: &mut ExprPattern)
where
    V: VisitMut + ?Sized,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with_mut(expr),
        ExprPattern::Like(expr) => v.visit_expr_like_mut(expr),
    }
}

pub fn visit_expr_project_mut<V>(v: &mut V, node: &mut ExprProject)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_projection_mut(&mut node.projection);
}

pub fn visit_filter_mut<V>(v: &mut V, node: &mut Filter)
where
    V: VisitMut + ?Sized,
{
    if let Some(expr) = &mut node.expr {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_condition_mut<V>(v: &mut V, node: &mut Condition)
where
    V: VisitMut + ?Sized,
{
    if let Some(expr) = &mut node.expr {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_insert_target_mut<V>(v: &mut V, node: &mut InsertTarget)
where
    V: VisitMut + ?Sized,
{
    if let InsertTarget::Scope(stmt) = node {
        v.visit_stmt_query_mut(stmt)
    }
}

pub fn visit_join_mut<V>(v: &mut V, node: &mut Join)
where
    V: VisitMut + ?Sized,
{
    v.visit_source_table_id_mut(&mut node.table);
    match &mut node.constraint {
        JoinOp::Left(expr) => v.visit_expr_mut(expr),
    }
}

pub fn visit_limit_mut<V>(v: &mut V, node: &mut Limit)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.limit);

    if let Some(offset) = &mut node.offset {
        v.visit_offset_mut(offset);
    }
}

pub fn visit_offset_mut<V>(v: &mut V, node: &mut Offset)
where
    V: VisitMut + ?Sized,
{
    match node {
        Offset::After(expr) => v.visit_expr_mut(expr),
        Offset::Count(expr) => v.visit_expr_mut(expr),
    }
}

pub fn visit_order_by_mut<V>(v: &mut V, node: &mut OrderBy)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.exprs {
        v.visit_order_by_expr_mut(expr);
    }
}

pub fn visit_order_by_expr_mut<V>(v: &mut V, node: &mut OrderByExpr)
where
    V: VisitMut + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_path_mut<V>(v: &mut V, node: &mut Path)
where
    V: VisitMut + ?Sized,
{
    v.visit_projection_mut(&mut node.projection);
}

pub fn visit_projection_mut<V>(v: &mut V, node: &mut Projection)
where
    V: VisitMut + ?Sized,
{
}

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

pub fn visit_source_mut<V>(v: &mut V, node: &mut Source)
where
    V: VisitMut + ?Sized,
{
    match node {
        Source::Model(source_model) => v.visit_source_model_mut(source_model),
        Source::Table(source_table) => v.visit_source_table_mut(source_table),
    }
}

pub fn visit_source_model_mut<V>(v: &mut V, node: &mut SourceModel)
where
    V: VisitMut + ?Sized,
{
    if let Some(association) = &mut node.via {
        v.visit_association_mut(association);
    }
}

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

pub fn visit_source_table_id_mut<V>(v: &mut V, node: &mut SourceTableId)
where
    V: VisitMut + ?Sized,
{
    // SourceTableId is just an index, nothing to visit
}

pub fn visit_table_factor_mut<V>(v: &mut V, node: &mut TableFactor)
where
    V: VisitMut + ?Sized,
{
    match node {
        TableFactor::Table(table_id) => v.visit_source_table_id_mut(table_id),
    }
}

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

pub fn visit_stmt_select_mut<V>(v: &mut V, node: &mut Select)
where
    V: VisitMut + ?Sized,
{
    v.visit_source_mut(&mut node.source);
    v.visit_filter_mut(&mut node.filter);
    v.visit_returning_mut(&mut node.returning);
}

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

pub fn visit_table_derived_mut<V>(v: &mut V, node: &mut TableDerived)
where
    V: VisitMut + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.subquery);
}

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

pub fn visit_table_with_joins_mut<V>(v: &mut V, node: &mut TableWithJoins)
where
    V: VisitMut + ?Sized,
{
    v.visit_table_factor_mut(&mut node.relation);
    for join in &mut node.joins {
        v.visit_join_mut(join);
    }
}

pub fn visit_type_mut<V>(v: &mut V, node: &mut Type)
where
    V: VisitMut + ?Sized,
{
    // Type is just type information, no traversal needed
}

pub fn visit_update_target_mut<V>(v: &mut V, node: &mut UpdateTarget)
where
    V: VisitMut + ?Sized,
{
    if let UpdateTarget::Query(stmt) = node {
        v.visit_stmt_query_mut(stmt)
    }
}

pub fn visit_value_mut<V>(v: &mut V, node: &mut Value)
where
    V: VisitMut + ?Sized,
{
    if let Value::Record(node) = node {
        v.visit_value_record(node);
    }
}

pub fn visit_value_record<V>(v: &mut V, node: &mut ValueRecord)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.fields {
        v.visit_value_mut(expr);
    }
}

pub fn visit_values_mut<V>(v: &mut V, node: &mut Values)
where
    V: VisitMut + ?Sized,
{
    for expr in &mut node.rows {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_with_mut<V>(v: &mut V, node: &mut With)
where
    V: VisitMut + ?Sized,
{
    for cte in &mut node.ctes {
        v.visit_cte_mut(cte);
    }
}

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
