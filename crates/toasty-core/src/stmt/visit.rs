#![allow(unused_variables)]

use super::{
    Assignment, Assignments, Association, Condition, Cte, Delete, Expr, ExprAnd, ExprAny, ExprArg,
    ExprBeginsWith, ExprBinaryOp, ExprCast, ExprColumn, ExprConcat, ExprEnum, ExprExists, ExprFunc,
    ExprInList, ExprInSubquery, ExprIsNull, ExprKey, ExprLike, ExprList, ExprMap, ExprNot, ExprOr,
    ExprPattern, ExprProject, ExprRecord, ExprReference, ExprSet, ExprSetOp, ExprStmt, ExprTy,
    Filter, FuncCount, FuncLastInsertId, Insert, InsertTarget, Join, JoinOp, Limit, Node, Offset,
    OrderBy, OrderByExpr, Path, Projection, Query, Returning, Select, Source, SourceModel,
    SourceTable, SourceTableId, Statement, TableDerived, TableFactor, TableRef, TableWithJoins,
    Type, Update, UpdateTarget, Value, ValueRecord, Values, With,
};

pub trait Visit {
    fn visit<N: Node>(&mut self, i: &N)
    where
        Self: Sized,
    {
        i.visit(self);
    }

    fn visit_assignment(&mut self, i: &Assignment) {
        visit_assignment(self, i);
    }

    fn visit_assignments(&mut self, i: &Assignments) {
        visit_assignments(self, i);
    }

    fn visit_association(&mut self, i: &Association) {
        visit_association(self, i);
    }

    fn visit_cte(&mut self, i: &Cte) {
        visit_cte(self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        visit_expr(self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd) {
        visit_expr_and(self, i);
    }

    fn visit_expr_any(&mut self, i: &ExprAny) {
        visit_expr_any(self, i);
    }

    fn visit_expr_arg(&mut self, i: &ExprArg) {
        visit_expr_arg(self, i);
    }

    fn visit_expr_begins_with(&mut self, i: &ExprBeginsWith) {
        visit_expr_begins_with(self, i);
    }

    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp) {
        visit_expr_binary_op(self, i);
    }

    fn visit_expr_cast(&mut self, i: &ExprCast) {
        visit_expr_cast(self, i);
    }

    fn visit_expr_column(&mut self, i: &ExprColumn) {
        visit_expr_column(self, i);
    }

    fn visit_expr_concat(&mut self, i: &ExprConcat) {
        visit_expr_concat(self, i);
    }

    fn visit_expr_default(&mut self) {
        visit_expr_default(self);
    }

    fn visit_expr_enum(&mut self, i: &ExprEnum) {
        visit_expr_enum(self, i);
    }

    fn visit_expr_exists(&mut self, i: &ExprExists) {
        visit_expr_exists(self, i);
    }

    fn visit_expr_func(&mut self, i: &ExprFunc) {
        visit_expr_func(self, i);
    }

    fn visit_expr_func_count(&mut self, i: &FuncCount) {
        visit_expr_func_count(self, i);
    }

    fn visit_expr_func_last_insert_id(&mut self, i: &FuncLastInsertId) {
        visit_expr_func_last_insert_id(self, i);
    }

    fn visit_expr_in_list(&mut self, i: &ExprInList) {
        visit_expr_in_list(self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery) {
        visit_expr_in_subquery(self, i);
    }

    fn visit_expr_is_null(&mut self, i: &ExprIsNull) {
        visit_expr_is_null(self, i);
    }

    fn visit_expr_like(&mut self, i: &ExprLike) {
        visit_expr_like(self, i);
    }

    fn visit_expr_key(&mut self, i: &ExprKey) {
        visit_expr_key(self, i);
    }

    fn visit_expr_map(&mut self, i: &ExprMap) {
        visit_expr_map(self, i);
    }

    fn visit_expr_not(&mut self, i: &ExprNot) {
        visit_expr_not(self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr) {
        visit_expr_or(self, i);
    }

    fn visit_expr_list(&mut self, i: &ExprList) {
        visit_expr_list(self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord) {
        visit_expr_record(self, i);
    }

    fn visit_expr_reference(&mut self, i: &ExprReference) {
        visit_expr_reference(self, i);
    }

    fn visit_expr_set(&mut self, i: &ExprSet) {
        visit_expr_set(self, i);
    }

    fn visit_expr_set_op(&mut self, i: &ExprSetOp) {
        visit_expr_set_op(self, i);
    }

    fn visit_expr_stmt(&mut self, i: &ExprStmt) {
        visit_expr_stmt(self, i);
    }

    fn visit_expr_ty(&mut self, i: &ExprTy) {
        visit_expr_ty(self, i);
    }

    fn visit_expr_pattern(&mut self, i: &ExprPattern) {
        visit_expr_pattern(self, i);
    }

    fn visit_filter(&mut self, i: &Filter) {
        visit_filter(self, i);
    }

    fn visit_condition(&mut self, i: &Condition) {
        visit_condition(self, i);
    }

    fn visit_expr_project(&mut self, i: &ExprProject) {
        visit_expr_project(self, i);
    }

    fn visit_insert_target(&mut self, i: &InsertTarget) {
        visit_insert_target(self, i);
    }

    fn visit_join(&mut self, i: &Join) {
        visit_join(self, i);
    }

    fn visit_limit(&mut self, i: &Limit) {
        visit_limit(self, i);
    }

    fn visit_offset(&mut self, i: &Offset) {
        visit_offset(self, i);
    }

    fn visit_order_by(&mut self, i: &OrderBy) {
        visit_order_by(self, i);
    }

    fn visit_order_by_expr(&mut self, i: &OrderByExpr) {
        visit_order_by_expr(self, i);
    }

    fn visit_path(&mut self, i: &Path) {
        visit_path(self, i);
    }

    fn visit_projection(&mut self, i: &Projection) {
        visit_projection(self, i);
    }

    fn visit_returning(&mut self, i: &Returning) {
        visit_returning(self, i);
    }

    fn visit_source(&mut self, i: &Source) {
        visit_source(self, i);
    }

    fn visit_source_model(&mut self, i: &SourceModel) {
        visit_source_model(self, i);
    }

    fn visit_source_table(&mut self, i: &SourceTable) {
        visit_source_table(self, i);
    }

    fn visit_source_table_id(&mut self, i: &SourceTableId) {
        visit_source_table_id(self, i);
    }

    fn visit_stmt(&mut self, i: &Statement) {
        visit_stmt(self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete) {
        visit_stmt_delete(self, i);
    }

    fn visit_stmt_insert(&mut self, i: &Insert) {
        visit_stmt_insert(self, i);
    }

    fn visit_stmt_query(&mut self, i: &Query) {
        visit_stmt_query(self, i);
    }

    fn visit_stmt_select(&mut self, i: &Select) {
        visit_stmt_select(self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update) {
        visit_stmt_update(self, i);
    }

    fn visit_table_derived(&mut self, i: &TableDerived) {
        visit_table_derived(self, i);
    }

    fn visit_table_ref(&mut self, i: &TableRef) {
        visit_table_ref(self, i);
    }

    fn visit_table_factor(&mut self, i: &TableFactor) {
        visit_table_factor(self, i);
    }

    fn visit_table_with_joins(&mut self, i: &TableWithJoins) {
        visit_table_with_joins(self, i);
    }

    fn visit_type(&mut self, i: &Type) {
        visit_type(self, i);
    }

    fn visit_update_target(&mut self, i: &UpdateTarget) {
        visit_update_target(self, i);
    }

    fn visit_value(&mut self, i: &Value) {
        visit_value(self, i);
    }

    fn visit_value_record(&mut self, i: &ValueRecord) {
        visit_value_record(self, i);
    }

    fn visit_values(&mut self, i: &Values) {
        visit_values(self, i);
    }

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

    fn visit_expr_concat(&mut self, i: &ExprConcat) {
        Visit::visit_expr_concat(&mut **self, i);
    }

    fn visit_expr_default(&mut self) {
        Visit::visit_expr_default(&mut **self);
    }

    fn visit_expr_enum(&mut self, i: &ExprEnum) {
        Visit::visit_expr_enum(&mut **self, i);
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

    fn visit_expr_like(&mut self, i: &ExprLike) {
        Visit::visit_expr_like(&mut **self, i);
    }

    fn visit_expr_key(&mut self, i: &ExprKey) {
        Visit::visit_expr_key(&mut **self, i);
    }

    fn visit_expr_map(&mut self, i: &ExprMap) {
        Visit::visit_expr_map(&mut **self, i);
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

    fn visit_expr_ty(&mut self, i: &ExprTy) {
        Visit::visit_expr_ty(&mut **self, i);
    }

    fn visit_expr_pattern(&mut self, i: &ExprPattern) {
        Visit::visit_expr_pattern(&mut **self, i);
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

    fn visit_offset(&mut self, i: &Offset) {
        Visit::visit_offset(&mut **self, i);
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

pub fn visit_assignment<V>(v: &mut V, node: &Assignment)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_assignments<V>(v: &mut V, node: &Assignments)
where
    V: Visit + ?Sized,
{
    for (_, assignment) in node.iter() {
        v.visit_assignment(assignment);
    }
}

pub fn visit_association<V>(v: &mut V, node: &Association)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.source);
}

pub fn visit_cte<V>(v: &mut V, node: &Cte)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.query);
}

pub fn visit_expr<V>(v: &mut V, node: &Expr)
where
    V: Visit + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and(expr),
        Expr::Any(expr) => v.visit_expr_any(expr),
        Expr::Arg(expr) => v.visit_expr_arg(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op(expr),
        Expr::Cast(expr) => v.visit_expr_cast(expr),
        Expr::Concat(expr) => v.visit_expr_concat(expr),
        Expr::Default => v.visit_expr_default(),
        Expr::Enum(expr) => v.visit_expr_enum(expr),
        Expr::Exists(expr) => v.visit_expr_exists(expr),
        Expr::Func(expr) => v.visit_expr_func(expr),
        Expr::InList(expr) => v.visit_expr_in_list(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery(expr),
        Expr::IsNull(expr) => v.visit_expr_is_null(expr),
        Expr::Key(expr) => v.visit_expr_key(expr),
        Expr::Map(expr) => v.visit_expr_map(expr),
        Expr::Not(expr) => v.visit_expr_not(expr),
        Expr::Or(expr) => v.visit_expr_or(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern(expr),
        Expr::Project(expr) => v.visit_expr_project(expr),
        Expr::Record(expr) => v.visit_expr_record(expr),
        Expr::Reference(expr) => v.visit_expr_reference(expr),
        Expr::List(expr) => v.visit_expr_list(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt(expr),
        Expr::Type(expr) => v.visit_expr_ty(expr),
        Expr::Value(expr) => v.visit_value(expr),
        // HAX
        Expr::ConcatStr(expr) => {
            for expr in &expr.exprs {
                v.visit_expr(expr);
            }
        }
        Expr::DecodeEnum(base, ..) => v.visit_expr(base),
    }
}

pub fn visit_expr_and<V>(v: &mut V, node: &ExprAnd)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_any<V>(v: &mut V, node: &ExprAny)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_expr_arg<V>(v: &mut V, node: &ExprArg)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_begins_with<V>(v: &mut V, node: &ExprBeginsWith)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_binary_op<V>(v: &mut V, node: &ExprBinaryOp)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.lhs);
    v.visit_expr(&node.rhs);
}

pub fn visit_expr_cast<V>(v: &mut V, node: &ExprCast)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_type(&node.ty);
}

pub fn visit_expr_column<V>(v: &mut V, node: &ExprColumn)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_concat<V>(v: &mut V, node: &ExprConcat)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_default<V>(v: &mut V)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_enum<V>(v: &mut V, node: &ExprEnum)
where
    V: Visit + ?Sized,
{
    v.visit_expr_record(&node.fields);
}

pub fn visit_expr_exists<V>(v: &mut V, node: &ExprExists)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.subquery);
}

pub fn visit_expr_func<V>(v: &mut V, node: &ExprFunc)
where
    V: Visit + ?Sized,
{
    match node {
        ExprFunc::Count(func) => v.visit_expr_func_count(func),
        ExprFunc::LastInsertId(func) => v.visit_expr_func_last_insert_id(func),
    }
}

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

pub fn visit_expr_func_last_insert_id<V>(_v: &mut V, _node: &FuncLastInsertId)
where
    V: Visit + ?Sized,
{
    // FuncLastInsertId has no fields to visit
}

pub fn visit_expr_in_list<V>(v: &mut V, node: &ExprInList)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.list);
}

pub fn visit_expr_in_subquery<V>(v: &mut V, node: &ExprInSubquery)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_stmt_query(&node.query);
}

pub fn visit_expr_is_null<V>(v: &mut V, node: &ExprIsNull)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_expr_like<V>(v: &mut V, node: &ExprLike)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_key<V>(v: &mut V, node: &ExprKey)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_map<V>(v: &mut V, node: &ExprMap)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_expr(&node.map);
}

pub fn visit_expr_not<V>(v: &mut V, node: &ExprNot)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_expr_or<V>(v: &mut V, node: &ExprOr)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_list<V>(v: &mut V, node: &ExprList)
where
    V: Visit + ?Sized,
{
    for expr in &node.items {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_record<V>(v: &mut V, node: &ExprRecord)
where
    V: Visit + ?Sized,
{
    for expr in &**node {
        v.visit_expr(expr);
    }
}

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

pub fn visit_expr_set_op<V>(v: &mut V, node: &ExprSetOp)
where
    V: Visit + ?Sized,
{
    for operand in &node.operands {
        v.visit_expr_set(operand);
    }
}

pub fn visit_expr_stmt<V>(v: &mut V, node: &ExprStmt)
where
    V: Visit + ?Sized,
{
    v.visit_stmt(&node.stmt);
}

pub fn visit_expr_ty<V>(v: &mut V, node: &ExprTy)
where
    V: Visit + ?Sized,
{
    v.visit_type(&node.ty);
}

pub fn visit_expr_pattern<V>(v: &mut V, node: &ExprPattern)
where
    V: Visit + ?Sized,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with(expr),
        ExprPattern::Like(expr) => v.visit_expr_like(expr),
    }
}

pub fn visit_expr_project<V>(v: &mut V, node: &ExprProject)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_projection(&node.projection);
}

pub fn visit_filter<V>(v: &mut V, node: &Filter)
where
    V: Visit + ?Sized,
{
    if let Some(expr) = &node.expr {
        v.visit_expr(expr);
    }
}

pub fn visit_condition<V>(v: &mut V, node: &Condition)
where
    V: Visit + ?Sized,
{
    if let Some(expr) = &node.expr {
        v.visit_expr(expr);
    }
}

pub fn visit_insert_target<V>(v: &mut V, node: &InsertTarget)
where
    V: Visit + ?Sized,
{
    if let InsertTarget::Scope(stmt) = node {
        v.visit_stmt_query(stmt);
    }
}

pub fn visit_join<V>(v: &mut V, node: &Join)
where
    V: Visit + ?Sized,
{
    v.visit_source_table_id(&node.table);
    match &node.constraint {
        JoinOp::Left(expr) => v.visit_expr(expr),
    }
}

pub fn visit_limit<V>(v: &mut V, node: &Limit)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.limit);

    if let Some(offset) = &node.offset {
        v.visit_offset(offset);
    }
}

pub fn visit_offset<V>(v: &mut V, node: &Offset)
where
    V: Visit + ?Sized,
{
    match node {
        Offset::After(expr) => v.visit_expr(expr),
        Offset::Count(expr) => v.visit_expr(expr),
    }
}

pub fn visit_order_by<V>(v: &mut V, node: &OrderBy)
where
    V: Visit + ?Sized,
{
    for expr in &node.exprs {
        v.visit_order_by_expr(expr);
    }
}

pub fn visit_order_by_expr<V>(v: &mut V, node: &OrderByExpr)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_path<V>(v: &mut V, node: &Path)
where
    V: Visit + ?Sized,
{
    v.visit_projection(&node.projection);
}

pub fn visit_projection<V>(v: &mut V, node: &Projection)
where
    V: Visit + ?Sized,
{
}

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

pub fn visit_source<V>(v: &mut V, node: &Source)
where
    V: Visit + ?Sized,
{
    match node {
        Source::Model(source_model) => v.visit_source_model(source_model),
        Source::Table(source_table) => v.visit_source_table(source_table),
    }
}

pub fn visit_source_model<V>(v: &mut V, node: &SourceModel)
where
    V: Visit + ?Sized,
{
    if let Some(association) = &node.via {
        v.visit_association(association);
    }
}

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

pub fn visit_source_table_id<V>(v: &mut V, node: &SourceTableId)
where
    V: Visit + ?Sized,
{
    // SourceTableId is just an index, nothing to visit
}

pub fn visit_table_factor<V>(v: &mut V, node: &TableFactor)
where
    V: Visit + ?Sized,
{
    match node {
        TableFactor::Table(table_id) => v.visit_source_table_id(table_id),
    }
}

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

pub fn visit_stmt_delete<V>(v: &mut V, node: &Delete)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.from);
    v.visit_filter(&node.filter);

    if let Some(returning) = &node.returning {
        v.visit_returning(returning);
    }
}

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

pub fn visit_stmt_select<V>(v: &mut V, node: &Select)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.source);
    v.visit_filter(&node.filter);
    v.visit_returning(&node.returning);
}

pub fn visit_stmt_update<V>(v: &mut V, node: &Update)
where
    V: Visit + ?Sized,
{
    v.visit_update_target(&node.target);
    v.visit_assignments(&node.assignments);
    v.visit_filter(&node.filter);
    v.visit_condition(&node.condition);
}

pub fn visit_table_derived<V>(v: &mut V, node: &TableDerived)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.subquery);
}

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

pub fn visit_table_with_joins<V>(v: &mut V, node: &TableWithJoins)
where
    V: Visit + ?Sized,
{
    v.visit_table_factor(&node.relation);
    for join in &node.joins {
        v.visit_join(join);
    }
}

pub fn visit_type<V>(v: &mut V, node: &Type)
where
    V: Visit + ?Sized,
{
    // Type is just type information, no traversal needed
}

pub fn visit_update_target<V>(v: &mut V, node: &UpdateTarget)
where
    V: Visit + ?Sized,
{
    if let UpdateTarget::Query(query) = node {
        v.visit_stmt_query(query)
    }
}

pub fn visit_value<V>(v: &mut V, node: &Value)
where
    V: Visit + ?Sized,
{
    if let Value::Record(node) = node {
        v.visit_value_record(node)
    }
}

pub fn visit_value_record<V>(v: &mut V, node: &ValueRecord)
where
    V: Visit + ?Sized,
{
    for value in node.iter() {
        v.visit_value(value);
    }
}

pub fn visit_values<V>(v: &mut V, node: &Values)
where
    V: Visit + ?Sized,
{
    for expr in &node.rows {
        v.visit_expr(expr);
    }
}

pub fn visit_with<V>(v: &mut V, node: &With)
where
    V: Visit + ?Sized,
{
    for cte in &node.ctes {
        v.visit_cte(cte);
    }
}

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
