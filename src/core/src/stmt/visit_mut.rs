#![allow(unused_variables)]

use super::*;

pub trait VisitMut<'stmt>: Sized {
    fn visit_mut<N: Node<'stmt>>(&mut self, i: &mut N) {
        i.visit_mut(self);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr<'stmt>) {
        visit_expr_mut(self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd<'stmt>) {
        visit_expr_and_mut(self, i);
    }

    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        visit_expr_arg_mut(self, i);
    }

    fn visit_expr_begins_with_mut(&mut self, i: &mut ExprBeginsWith<'stmt>) {
        visit_expr_begins_with_mut(self, i);
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp<'stmt>) {
        visit_expr_binary_op_mut(self, i);
    }

    fn visit_expr_concat_mut(&mut self, i: &mut ExprConcat<'stmt>) {
        visit_expr_concat_mut(self, i);
    }

    fn visit_expr_enum_mut(&mut self, i: &mut ExprEnum<'stmt>) {
        visit_expr_enum_mut(self, i);
    }

    fn visit_expr_in_list_mut(&mut self, i: &mut ExprInList<'stmt>) {
        visit_expr_in_list_mut(self, i);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery<'stmt>) {
        visit_expr_in_subquery_mut(self, i);
    }

    fn visit_expr_like_mut(&mut self, i: &mut ExprLike<'stmt>) {
        visit_expr_like_mut(self, i);
    }

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr<'stmt>) {
        visit_expr_or_mut(self, i);
    }

    fn visit_expr_list_mut(&mut self, i: &mut Vec<Expr<'stmt>>) {
        visit_expr_list_mut(self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord<'stmt>) {
        visit_expr_record_mut(self, i);
    }

    fn visit_expr_set_mut(&mut self, i: &mut ExprSet<'stmt>) {
        visit_expr_set_mut(self, i);
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp<'stmt>) {
        visit_expr_set_op_mut(self, i);
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt<'stmt>) {
        visit_expr_stmt_mut(self, i);
    }

    fn visit_expr_ty_mut(&mut self, i: &mut ExprTy) {
        visit_expr_ty_mut(self, i);
    }

    fn visit_expr_pattern_mut(&mut self, i: &mut ExprPattern<'stmt>) {
        visit_expr_pattern_mut(self, i);
    }

    fn visit_expr_project_mut(&mut self, i: &mut ExprProject<'stmt>) {
        visit_expr_project_mut(self, i);
    }

    fn visit_project_base_mut(&mut self, i: &mut ProjectBase<'stmt>) {
        visit_project_base_mut(self, i);
    }

    fn visit_projection_mut(&mut self, i: &mut Projection) {
        visit_projection_mut(self, i);
    }

    fn visit_returning_mut(&mut self, i: &mut Returning<'stmt>) {
        visit_returning_mut(self, i);
    }

    fn visit_stmt_mut(&mut self, i: &mut Statement<'stmt>) {
        visit_stmt_mut(self, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut Select<'stmt>) {
        visit_stmt_select_mut(self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert<'stmt>) {
        visit_stmt_insert_mut(self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query<'stmt>) {
        visit_stmt_query_mut(self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update<'stmt>) {
        visit_stmt_update_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete<'stmt>) {
        visit_stmt_delete_mut(self, i);
    }

    fn visit_stmt_link_mut(&mut self, i: &mut Link<'stmt>) {
        visit_stmt_link_mut(self, i);
    }

    fn visit_stmt_unlink_mut(&mut self, i: &mut Unlink<'stmt>) {
        visit_stmt_unlink_mut(self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value<'stmt>) {
        visit_value_mut(self, i);
    }
}

impl<'stmt, V: VisitMut<'stmt>> VisitMut<'stmt> for &mut V {
    fn visit_expr_mut(&mut self, i: &mut Expr<'stmt>) {
        VisitMut::visit_expr_mut(&mut **self, i);
    }

    fn visit_expr_and_mut(&mut self, i: &mut ExprAnd<'stmt>) {
        VisitMut::visit_expr_and_mut(&mut **self, i);
    }

    fn visit_expr_arg_mut(&mut self, i: &mut ExprArg) {
        VisitMut::visit_expr_arg_mut(&mut **self, i);
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut ExprBinaryOp<'stmt>) {
        VisitMut::visit_expr_binary_op_mut(&mut **self, i);
    }

    fn visit_expr_concat_mut(&mut self, i: &mut ExprConcat<'stmt>) {
        VisitMut::visit_expr_concat_mut(&mut **self, i);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut ExprInSubquery<'stmt>) {
        VisitMut::visit_expr_in_subquery_mut(&mut **self, i);
    }

    fn visit_expr_or_mut(&mut self, i: &mut ExprOr<'stmt>) {
        VisitMut::visit_expr_or_mut(&mut **self, i);
    }

    fn visit_expr_set_mut(&mut self, i: &mut ExprSet<'stmt>) {
        VisitMut::visit_expr_set_mut(&mut **self, i);
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut ExprSetOp<'stmt>) {
        VisitMut::visit_expr_set_op_mut(&mut **self, i);
    }

    fn visit_expr_record_mut(&mut self, i: &mut ExprRecord<'stmt>) {
        VisitMut::visit_expr_record_mut(&mut **self, i);
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut ExprStmt<'stmt>) {
        VisitMut::visit_expr_stmt_mut(&mut **self, i);
    }

    fn visit_expr_ty_mut(&mut self, i: &mut ExprTy) {
        VisitMut::visit_expr_ty_mut(&mut **self, i);
    }

    fn visit_stmt_mut(&mut self, i: &mut Statement<'stmt>) {
        VisitMut::visit_stmt_mut(&mut **self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut Query<'stmt>) {
        VisitMut::visit_stmt_query_mut(&mut **self, i);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut Insert<'stmt>) {
        VisitMut::visit_stmt_insert_mut(&mut **self, i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut Update<'stmt>) {
        VisitMut::visit_stmt_update_mut(&mut **self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut Delete<'stmt>) {
        VisitMut::visit_stmt_delete_mut(&mut **self, i);
    }

    fn visit_value_mut(&mut self, i: &mut Value<'stmt>) {
        VisitMut::visit_value_mut(&mut **self, i);
    }
}

pub fn visit_expr_mut<'stmt, V>(v: &mut V, node: &mut Expr<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and_mut(expr),
        Expr::Arg(expr) => v.visit_expr_arg_mut(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op_mut(expr),
        Expr::Concat(expr) => v.visit_expr_concat_mut(expr),
        Expr::Enum(expr) => v.visit_expr_enum_mut(expr),
        Expr::InList(expr) => v.visit_expr_in_list_mut(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery_mut(expr),
        Expr::Or(expr) => v.visit_expr_or_mut(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern_mut(expr),
        Expr::Project(expr) => v.visit_expr_project_mut(expr),
        Expr::Record(expr) => v.visit_expr_record_mut(expr),
        Expr::List(expr) => v.visit_expr_list_mut(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt_mut(expr),
        Expr::Type(expr) => v.visit_expr_ty_mut(expr),
        Expr::Value(expr) => v.visit_value_mut(expr),
    }
}

pub fn visit_expr_and_mut<'stmt, V>(v: &mut V, node: &mut ExprAnd<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_arg_mut<'stmt, V>(v: &mut V, node: &mut ExprArg)
where
    V: VisitMut<'stmt> + ?Sized,
{
}

pub fn visit_expr_begins_with_mut<'stmt, V>(v: &mut V, node: &mut ExprBeginsWith<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_binary_op_mut<'stmt, V>(v: &mut V, node: &mut ExprBinaryOp<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.lhs);
    v.visit_expr_mut(&mut node.rhs);
}

pub fn visit_expr_concat_mut<'stmt, V>(v: &mut V, node: &mut ExprConcat<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_enum_mut<'stmt, V>(v: &mut V, node: &mut ExprEnum<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_record_mut(&mut node.fields);
}

pub fn visit_expr_in_list_mut<'stmt, V>(v: &mut V, node: &mut ExprInList<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.list);
}

pub fn visit_expr_in_subquery_mut<'stmt, V>(v: &mut V, node: &mut ExprInSubquery<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_stmt_query_mut(&mut node.query);
}

pub fn visit_expr_like_mut<'stmt, V>(v: &mut V, node: &mut ExprLike<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.expr);
    v.visit_expr_mut(&mut node.pattern);
}

pub fn visit_expr_or_mut<'stmt, V>(v: &mut V, node: &mut ExprOr<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_list_mut<'stmt, V>(v: &mut V, node: &mut Vec<Expr<'stmt>>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for e in node {
        v.visit_expr_mut(e);
    }
}

pub fn visit_expr_record_mut<'stmt, V>(v: &mut V, node: &mut ExprRecord<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for expr in &mut **node {
        v.visit_expr_mut(expr);
    }
}

pub fn visit_expr_set_mut<'stmt, V>(v: &mut V, node: &mut ExprSet<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select_mut(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op_mut(expr),
        ExprSet::Values(_) => todo!(),
    }
}

pub fn visit_expr_set_op_mut<'stmt, V>(v: &mut V, node: &mut ExprSetOp<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    for operand in &mut node.operands {
        v.visit_expr_set_mut(operand);
    }
}

pub fn visit_expr_stmt_mut<'stmt, V>(v: &mut V, node: &mut ExprStmt<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_mut(&mut node.stmt);
}

pub fn visit_expr_ty_mut<'stmt, V>(v: &mut V, node: &mut ExprTy)
where
    V: VisitMut<'stmt> + ?Sized,
{
}

pub fn visit_expr_pattern_mut<'stmt, V>(v: &mut V, node: &mut ExprPattern<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with_mut(expr),
        ExprPattern::Like(expr) => v.visit_expr_like_mut(expr),
    }
}

pub fn visit_expr_project_mut<'stmt, V>(v: &mut V, node: &mut ExprProject<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_project_base_mut(&mut node.base);
    v.visit_projection_mut(&mut node.projection);
}

pub fn visit_project_base_mut<'stmt, V>(v: &mut V, node: &mut ProjectBase<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        ProjectBase::ExprSelf => {}
        ProjectBase::Expr(e) => v.visit_expr_mut(e),
    }
}

pub fn visit_projection_mut<'stmt, V>(v: &mut V, node: &mut Projection)
where
    V: VisitMut<'stmt> + ?Sized,
{
}

pub fn visit_returning_mut<'stmt, V>(v: &mut V, node: &mut Returning<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        Returning::Star => {}
        Returning::Expr(expr) => v.visit_expr_mut(expr),
    }
}

pub fn visit_stmt_mut<'stmt, V>(v: &mut V, node: &mut Statement<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    match node {
        Statement::Delete(stmt) => v.visit_stmt_delete_mut(stmt),
        Statement::Link(_) => todo!(),
        Statement::Insert(stmt) => v.visit_stmt_insert_mut(stmt),
        Statement::Query(stmt) => v.visit_stmt_query_mut(stmt),
        Statement::Unlink(_) => todo!(),
        Statement::Update(stmt) => v.visit_stmt_update_mut(stmt),
    }
}

pub fn visit_stmt_select_mut<'stmt, V>(v: &mut V, node: &mut Select<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_mut(&mut node.filter);
    v.visit_returning_mut(&mut node.returning);
}

pub fn visit_stmt_insert_mut<'stmt, V>(v: &mut V, node: &mut Insert<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.scope);
    v.visit_expr_mut(&mut node.values);

    if let Some(returning) = &mut node.returning {
        v.visit_returning_mut(returning);
    }
}

pub fn visit_stmt_query_mut<'stmt, V>(v: &mut V, node: &mut Query<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_expr_set_mut(&mut node.body);
}

pub fn visit_stmt_update_mut<'stmt, V>(v: &mut V, node: &mut Update<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.selection);
}

pub fn visit_stmt_delete_mut<'stmt, V>(v: &mut V, node: &mut Delete<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.selection);
}

pub fn visit_stmt_link_mut<'stmt, V>(v: &mut V, node: &mut Link<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.source);
    v.visit_stmt_query_mut(&mut node.target);
}

pub fn visit_stmt_unlink_mut<'stmt, V>(v: &mut V, node: &mut Unlink<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
    v.visit_stmt_query_mut(&mut node.source);
    v.visit_stmt_query_mut(&mut node.target);
}

pub fn visit_value_mut<'stmt, V>(v: &mut V, node: &mut Value<'stmt>)
where
    V: VisitMut<'stmt> + ?Sized,
{
}

pub fn for_each_expr_mut<'stmt, F>(node: &mut impl Node<'stmt>, f: F)
where
    F: FnMut(&mut Expr<'stmt>),
{
    struct ForEach<F> {
        f: F,
    }

    impl<'stmt, F> VisitMut<'stmt> for ForEach<F>
    where
        F: FnMut(&mut Expr<'stmt>),
    {
        fn visit_expr_mut(&mut self, node: &mut Expr<'stmt>) {
            visit_expr_mut(self, node);
            (self.f)(node);
        }
    }

    node.visit_mut(ForEach { f });
}
