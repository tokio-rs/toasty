#![allow(unused_variables)]

use super::*;

pub trait VisitMut: Sized {
    fn visit_mut<N: Node>(&mut self, i: &mut N) {
        i.visit_mut(self);
    }

    fn visit_assignment_mut(&mut self, i: &mut Assignment) {
        visit_assignment_mut(self, i);
    }

    fn visit_assignments_mut(&mut self, i: &mut Assignments) {
        visit_assignments_mut(self, i);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        visit_expr_mut(self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd) {
        visit_expr_and_mut(self, i);
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

    fn visit_expr_enum_mut(&mut self, i: &mut ExprEnum) {
        visit_expr_enum_mut(self, i);
    }

    fn visit_expr_field_mut(&mut self, i: &mut ExprField) {
        visit_expr_field_mut(self, i);
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

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr) {
        visit_expr_or_mut(self, i);
    }

    fn visit_expr_list_mut(&mut self, i: &mut ExprList) {
        visit_expr_list_mut(self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord) {
        visit_expr_record_mut(self, i);
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

    fn visit_expr_project_mut(&mut self, i: &mut ExprProject) {
        visit_expr_project_mut(self, i);
    }

    fn visit_insert_target_mut(&mut self, i: &mut InsertTarget) {
        visit_insert_target_mut(self, i);
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

    fn visit_stmt_mut(&mut self, i: &mut Statement) {
        visit_stmt_mut(self, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut Select) {
        visit_stmt_select_mut(self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert) {
        visit_stmt_insert_mut(self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query) {
        visit_stmt_query_mut(self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update) {
        visit_stmt_update_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete) {
        visit_stmt_delete_mut(self, i);
    }

    fn visit_update_target_mut(&mut self, i: &mut UpdateTarget) {
        visit_update_target_mut(self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value) {
        visit_value_mut(self, i);
    }

    fn visit_values_mut(&mut self, i: &mut Values) {
        visit_values_mut(self, i);
    }
}

impl<V: VisitMut> VisitMut for &mut V {
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        VisitMut::visit_expr_mut(&mut **self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd) {
        VisitMut::visit_expr_and_mut(&mut **self, i);
    }

    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        VisitMut::visit_expr_arg_mut(&mut **self, i);
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp) {
        VisitMut::visit_expr_binary_op_mut(&mut **self, i);
    }

    fn visit_expr_concat_mut(&mut self, i: &mut ExprConcat) {
        VisitMut::visit_expr_concat_mut(&mut **self, i);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery) {
        VisitMut::visit_expr_in_subquery_mut(&mut **self, i);
    }

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr) {
        VisitMut::visit_expr_or_mut(&mut **self, i);
    }

    fn visit_expr_set_mut(&mut self, i: &mut ExprSet) {
        VisitMut::visit_expr_set_mut(&mut **self, i);
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp) {
        VisitMut::visit_expr_set_op_mut(&mut **self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord) {
        VisitMut::visit_expr_record_mut(&mut **self, i);
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt) {
        VisitMut::visit_expr_stmt_mut(&mut **self, i);
    }

    fn visit_expr_ty_mut(&mut self, i: &mut ExprTy) {
        VisitMut::visit_expr_ty_mut(&mut **self, i);
    }

    fn visit_stmt_mut(&mut self, i: &mut Statement) {
        VisitMut::visit_stmt_mut(&mut **self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query) {
        VisitMut::visit_stmt_query_mut(&mut **self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert) {
        VisitMut::visit_stmt_insert_mut(&mut **self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update) {
        VisitMut::visit_stmt_update_mut(&mut **self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete) {
        VisitMut::visit_stmt_delete_mut(&mut **self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value) {
        VisitMut::visit_value_mut(&mut **self, i);
    }
}

pub fn visit_assignment_mut<V>(v: &mut V, node: &mut Assignment)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_assignments_mut<V>(v: &mut V, node: &mut Assignments)
where
    V: VisitMut,
{
    for (_, assignment) in node.iter_mut() {
        v.visit_assignment_mut(assignment);
    }
}

pub fn visit_expr_mut<V>(v: &mut V, node: &mut Expr)
where
    V: VisitMut,
{
    match node {
        Expr::And(expr) => v.visit_expr_and_mut(expr),
        Expr::Arg(expr) => v.visit_expr_arg_mut(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op_mut(expr),
        Expr::Cast(expr) => v.visit_expr_cast_mut(expr),
        Expr::Column(expr) => v.visit_expr_column_mut(expr),
        Expr::Concat(expr) => v.visit_expr_concat_mut(expr),
        Expr::Enum(expr) => v.visit_expr_enum_mut(expr),
        Expr::Field(expr) => v.visit_expr_field_mut(expr),
        Expr::InList(expr) => v.visit_expr_in_list_mut(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery_mut(expr),
        Expr::IsNull(expr) => v.visit_expr_is_null_mut(expr),
        Expr::Key(expr) => v.visit_expr_key_mut(expr),
        Expr::Map(expr) => v.visit_expr_map_mut(expr),
        Expr::Or(expr) => v.visit_expr_or_mut(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern_mut(expr),
        Expr::Project(expr) => v.visit_expr_project_mut(expr),
        Expr::Record(expr) => v.visit_expr_record_mut(expr),
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
        Expr::DecodeEnum(base, ..) => v.visit_expr_mut(base),
    }
}

pub fn visit_expr_and_mut<V>(v: &mut V, node: &mut ExprAnd)
where
    V: VisitMut,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_arg_mut<V>(v: &mut V, node: &mut ExprArg)
where
    V: VisitMut,
{
}

pub fn visit_expr_begins_with_mut<V>(v: &mut V, node: &mut ExprBeginsWith)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_binary_op_mut<V>(v: &mut V, node: &mut ExprBinaryOp)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.lhs);
    v.visit_expr_mut(&mut node.rhs);
}

pub fn visit_expr_cast_mut<V>(v: &mut V, node: &mut ExprCast)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_expr_column_mut<V>(v: &mut V, node: &mut ExprColumn)
where
    V: VisitMut,
{
}

pub fn visit_expr_concat_mut<V>(v: &mut V, node: &mut ExprConcat)
where
    V: VisitMut,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_enum_mut<V>(v: &mut V, node: &mut ExprEnum)
where
    V: VisitMut,
{
    v.visit_expr_record_mut(&mut node.fields);
}

pub fn visit_expr_field_mut<V>(_v: &mut V, _node: &mut ExprField)
where
    V: VisitMut,
{
}

pub fn visit_expr_in_list_mut<V>(v: &mut V, node: &mut ExprInList)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.list);
}

pub fn visit_expr_in_subquery_mut<V>(v: &mut V, node: &mut ExprInSubquery)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_stmt_query_mut(&mut node.query);
}

pub fn visit_expr_is_null_mut<V>(v: &mut V, node: &mut ExprIsNull)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
}

pub fn visit_expr_like_mut<V>(v: &mut V, node: &mut ExprLike)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_key_mut<V>(v: &mut V, node: &mut ExprKey)
where
    V: VisitMut,
{
}

pub fn visit_expr_map_mut<V>(v: &mut V, node: &mut ExprMap)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_expr_mut(&mut node.map);
}

pub fn visit_expr_or_mut<V>(v: &mut V, node: &mut ExprOr)
where
    V: VisitMut,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_list_mut<V>(v: &mut V, node: &mut ExprList)
where
    V: VisitMut,
{
    for e in &mut node.items {
        v.visit_expr_mut(e);
    }
}

pub fn visit_expr_record_mut<V>(v: &mut V, node: &mut ExprRecord)
where
    V: VisitMut,
{
    for expr in &mut **node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_set_mut<V>(v: &mut V, node: &mut ExprSet)
where
    V: VisitMut,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select_mut(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op_mut(expr),
        ExprSet::Values(expr) => v.visit_values_mut(expr),
    }
}

pub fn visit_expr_set_op_mut<V>(v: &mut V, node: &mut ExprSetOp)
where
    V: VisitMut,
{
    for operand in &mut node.operands {
        v.visit_expr_set_mut(operand);
    }
}

pub fn visit_expr_stmt_mut<V>(v: &mut V, node: &mut ExprStmt)
where
    V: VisitMut,
{
    v.visit_stmt_mut(&mut node.stmt);
}

pub fn visit_expr_ty_mut<V>(v: &mut V, node: &mut ExprTy)
where
    V: VisitMut,
{
}

pub fn visit_expr_pattern_mut<V>(v: &mut V, node: &mut ExprPattern)
where
    V: VisitMut,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with_mut(expr),
        ExprPattern::Like(expr) => v.visit_expr_like_mut(expr),
    }
}

pub fn visit_expr_project_mut<V>(v: &mut V, node: &mut ExprProject)
where
    V: VisitMut,
{
    v.visit_expr_mut(&mut node.base);
    v.visit_projection_mut(&mut node.projection);
}

pub fn visit_insert_target_mut<V>(v: &mut V, node: &mut InsertTarget)
where
    V: VisitMut,
{
    if let InsertTarget::Scope(stmt) = node {
        v.visit_stmt_query_mut(stmt)
    }
}

pub fn visit_projection_mut<V>(v: &mut V, node: &mut Projection)
where
    V: VisitMut,
{
}

pub fn visit_returning_mut<V>(v: &mut V, node: &mut Returning)
where
    V: VisitMut,
{
    match node {
        Returning::Star | Returning::Changed => {}
        Returning::Expr(expr) => v.visit_expr_mut(expr),
    }
}

pub fn visit_source_mut<V>(_v: &mut V, _node: &mut Source)
where
    V: VisitMut,
{
}

pub fn visit_stmt_mut<V>(v: &mut V, node: &mut Statement)
where
    V: VisitMut,
{
    match node {
        Statement::Delete(stmt) => v.visit_stmt_delete_mut(stmt),
        Statement::Insert(stmt) => v.visit_stmt_insert_mut(stmt),
        Statement::Query(stmt) => v.visit_stmt_query_mut(stmt),
        Statement::Update(stmt) => v.visit_stmt_update_mut(stmt),
    }
}

pub fn visit_stmt_select_mut<V>(v: &mut V, node: &mut Select)
where
    V: VisitMut,
{
    v.visit_source_mut(&mut node.source);
    v.visit_expr_mut(&mut node.filter);
    v.visit_returning_mut(&mut node.returning);
}

pub fn visit_stmt_insert_mut<V>(v: &mut V, node: &mut Insert)
where
    V: VisitMut,
{
    v.visit_insert_target_mut(&mut node.target);
    v.visit_stmt_query_mut(&mut node.source);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

pub fn visit_stmt_query_mut<V>(v: &mut V, node: &mut Query)
where
    V: VisitMut,
{
    v.visit_expr_set_mut(&mut node.body);
}

pub fn visit_stmt_update_mut<V>(v: &mut V, node: &mut Update)
where
    V: VisitMut,
{
    v.visit_update_target_mut(&mut node.target);
    v.visit_assignments_mut(&mut node.assignments);

    if let Some(expr) = &mut node.filter {
        v.visit_expr_mut(expr);
    }

    if let Some(expr) = &mut node.condition {
        v.visit_expr_mut(expr);
    }

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

pub fn visit_stmt_delete_mut<V>(v: &mut V, node: &mut Delete)
where
    V: VisitMut,
{
    v.visit_source_mut(&mut node.from);
    v.visit_expr_mut(&mut node.filter);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

pub fn visit_update_target_mut<V>(v: &mut V, node: &mut UpdateTarget)
where
    V: VisitMut,
{
}

pub fn visit_value_mut<V>(v: &mut V, node: &mut Value)
where
    V: VisitMut,
{
}

pub fn visit_values_mut<V>(v: &mut V, node: &mut Values)
where
    V: VisitMut,
{
    for expr in &mut node.rows {
        v.visit_expr_mut(expr);
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
