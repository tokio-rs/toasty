#![allow(unused_variables)]

use super::*;

pub trait Visit<'stmt>: Sized {
    fn visit<N: Node<'stmt>>(&mut self, i: &N) {
        i.visit(self);
    }

    fn visit_expr(&mut self, i: &Expr<'stmt>) {
        visit_expr(self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd<'stmt>) {
        visit_expr_and(self, i);
    }

    fn visit_expr_arg(&mut self, i: &ExprArg) {
        visit_expr_arg(self, i);
    }

    fn visit_expr_begins_with(&mut self, i: &ExprBeginsWith<'stmt>) {
        visit_expr_begins_with(self, i);
    }

    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp<'stmt>) {
        visit_expr_binary_op(self, i);
    }

    fn visit_expr_column(&mut self, i: &ExprColumn) {
        visit_expr_column(self, i);
    }

    fn visit_expr_concat(&mut self, i: &ExprConcat<'stmt>) {
        visit_expr_concat(self, i);
    }

    fn visit_expr_enum(&mut self, i: &ExprEnum<'stmt>) {
        visit_expr_enum(self, i);
    }

    fn visit_expr_field(&mut self, i: &ExprField) {
        visit_expr_field(self, i);
    }

    fn visit_expr_in_list(&mut self, i: &ExprInList<'stmt>) {
        visit_expr_in_list(self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery<'stmt>) {
        visit_expr_in_subquery(self, i);
    }

    fn visit_expr_like(&mut self, i: &ExprLike<'stmt>) {
        visit_expr_like(self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr<'stmt>) {
        visit_expr_or(self, i);
    }

    fn visit_expr_list(&mut self, i: &Vec<Expr<'stmt>>) {
        visit_expr_list(self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord<'stmt>) {
        visit_expr_record(self, i);
    }

    fn visit_expr_set(&mut self, i: &ExprSet<'stmt>) {
        visit_expr_set(self, i);
    }

    fn visit_expr_set_op(&mut self, i: &ExprSetOp<'stmt>) {
        visit_expr_set_op(self, i);
    }

    fn visit_expr_stmt(&mut self, i: &ExprStmt<'stmt>) {
        visit_expr_stmt(self, i);
    }

    fn visit_expr_ty(&mut self, i: &ExprTy) {
        visit_expr_ty(self, i);
    }

    fn visit_expr_pattern(&mut self, i: &ExprPattern<'stmt>) {
        visit_expr_pattern(self, i);
    }

    fn visit_expr_project(&mut self, i: &ExprProject<'stmt>) {
        visit_expr_project(self, i);
    }

    fn visit_projection(&mut self, i: &Projection) {
        visit_projection(self, i);
    }

    fn visit_returning(&mut self, i: &Returning<'stmt>) {
        visit_returning(self, i);
    }

    fn visit_stmt(&mut self, i: &Statement<'stmt>) {
        visit_stmt(self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete<'stmt>) {
        visit_stmt_delete(self, i);
    }

    fn visit_stmt_link(&mut self, i: &Link<'stmt>) {
        visit_stmt_link(self, i);
    }

    fn visit_stmt_insert(&mut self, i: &Insert<'stmt>) {
        visit_stmt_insert(self, i);
    }

    fn visit_stmt_query(&mut self, i: &Query<'stmt>) {
        visit_stmt_query(self, i);
    }

    fn visit_stmt_select(&mut self, i: &Select<'stmt>) {
        visit_stmt_select(self, i);
    }

    fn visit_stmt_unlink(&mut self, i: &Unlink<'stmt>) {
        visit_stmt_unlink(self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update<'stmt>) {
        visit_stmt_update(self, i);
    }

    fn visit_value(&mut self, i: &Value<'stmt>) {
        visit_value(self, i);
    }

    fn visit_values(&mut self, i: &Values<'stmt>) {
        visit_values(self, i);
    }

    fn visit_value_record(&mut self, i: &Record<'stmt>) {
        visit_value_record(self, i);
    }
}

impl<'stmt, V: Visit<'stmt>> Visit<'stmt> for &mut V {
    fn visit_expr(&mut self, i: &Expr<'stmt>) {
        Visit::visit_expr(&mut **self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd<'stmt>) {
        Visit::visit_expr_and(&mut **self, i);
    }

    fn visit_expr_arg(&mut self, i: &ExprArg) {
        Visit::visit_expr_arg(&mut **self, i);
    }

    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp<'stmt>) {
        Visit::visit_expr_binary_op(&mut **self, i);
    }

    fn visit_expr_concat(&mut self, i: &ExprConcat<'stmt>) {
        Visit::visit_expr_concat(&mut **self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery<'stmt>) {
        Visit::visit_expr_in_subquery(&mut **self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr<'stmt>) {
        Visit::visit_expr_or(&mut **self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord<'stmt>) {
        Visit::visit_expr_record(&mut **self, i);
    }

    fn visit_expr_set(&mut self, i: &ExprSet<'stmt>) {
        Visit::visit_expr_set(&mut **self, i);
    }

    fn visit_expr_set_op(&mut self, i: &ExprSetOp<'stmt>) {
        Visit::visit_expr_set_op(&mut **self, i);
    }

    fn visit_expr_stmt(&mut self, i: &ExprStmt<'stmt>) {
        Visit::visit_expr_stmt(&mut **self, i);
    }

    fn visit_expr_ty(&mut self, i: &ExprTy) {
        Visit::visit_expr_ty(&mut **self, i);
    }

    fn visit_stmt(&mut self, i: &Statement<'stmt>) {
        Visit::visit_stmt(&mut **self, i);
    }

    fn visit_stmt_query(&mut self, i: &Query<'stmt>) {
        Visit::visit_stmt_query(&mut **self, i);
    }

    fn visit_stmt_insert(&mut self, i: &Insert<'stmt>) {
        Visit::visit_stmt_insert(&mut **self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update<'stmt>) {
        Visit::visit_stmt_update(&mut **self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete<'stmt>) {
        Visit::visit_stmt_delete(&mut **self, i);
    }

    fn visit_value(&mut self, i: &Value<'stmt>) {
        Visit::visit_value(&mut **self, i);
    }

    fn visit_values(&mut self, i: &Values<'stmt>) {
        Visit::visit_values(&mut **self, i);
    }
}

pub fn visit_expr<'stmt, V>(v: &mut V, node: &Expr<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and(expr),
        Expr::Arg(expr) => v.visit_expr_arg(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op(expr),
        Expr::Column(expr) => v.visit_expr_column(expr),
        Expr::Concat(expr) => v.visit_expr_concat(expr),
        Expr::Enum(expr) => v.visit_expr_enum(expr),
        Expr::Field(expr) => v.visit_expr_field(expr),
        Expr::InList(expr) => v.visit_expr_in_list(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery(expr),
        Expr::Or(expr) => v.visit_expr_or(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern(expr),
        Expr::Project(expr) => v.visit_expr_project(expr),
        Expr::Record(expr) => v.visit_expr_record(expr),
        Expr::List(expr) => v.visit_expr_list(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt(expr),
        Expr::Type(expr) => v.visit_expr_ty(expr),
        Expr::Value(expr) => v.visit_value(expr),
    }
}

pub fn visit_expr_and<'stmt, V>(v: &mut V, node: &ExprAnd<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_arg<'stmt, V>(v: &mut V, node: &ExprArg)
where
    V: Visit<'stmt> + ?Sized,
{
}

pub fn visit_expr_begins_with<'stmt, V>(v: &mut V, node: &ExprBeginsWith<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_binary_op<'stmt, V>(v: &mut V, node: &ExprBinaryOp<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.lhs);
    v.visit_expr(&node.rhs);
}

pub fn visit_expr_column<'stmt, V>(v: &mut V, node: &ExprColumn)
where
    V: Visit<'stmt> + ?Sized,
{
}

pub fn visit_expr_concat<'stmt, V>(v: &mut V, node: &ExprConcat<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_enum<'stmt, V>(v: &mut V, node: &ExprEnum<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr_record(&node.fields);
}

pub fn visit_expr_field<'stmt, V>(_v: &mut V, _node: &ExprField)
where
    V: Visit<'stmt> + ?Sized,
{
}

pub fn visit_expr_in_list<'stmt, V>(v: &mut V, node: &ExprInList<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.list);
}

pub fn visit_expr_in_subquery<'stmt, V>(v: &mut V, node: &ExprInSubquery<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_stmt_query(&node.query);
}

pub fn visit_expr_like<'stmt, V>(v: &mut V, node: &ExprLike<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_or<'stmt, V>(v: &mut V, node: &ExprOr<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_list<'stmt, V>(v: &mut V, node: &Vec<Expr<'stmt>>)
where
    V: Visit<'stmt> + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_record<'stmt, V>(v: &mut V, node: &ExprRecord<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for expr in &**node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_set<'stmt, V>(v: &mut V, node: &ExprSet<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op(expr),
        ExprSet::Values(expr) => v.visit_values(expr),
    }
}

pub fn visit_expr_set_op<'stmt, V>(v: &mut V, node: &ExprSetOp<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for operand in &node.operands {
        v.visit_expr_set(operand);
    }
}

pub fn visit_expr_stmt<'stmt, V>(v: &mut V, node: &ExprStmt<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt(&node.stmt);
}

pub fn visit_expr_ty<'stmt, V>(v: &mut V, node: &ExprTy)
where
    V: Visit<'stmt> + ?Sized,
{
}

pub fn visit_expr_pattern<'stmt, V>(v: &mut V, node: &ExprPattern<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with(expr),
        ExprPattern::Like(expr) => v.visit_expr_like(expr),
    }
}

pub fn visit_expr_project<'stmt, V>(v: &mut V, node: &ExprProject<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_projection(&node.projection);
}

pub fn visit_projection<'stmt, V>(v: &mut V, node: &Projection)
where
    V: Visit<'stmt> + ?Sized,
{
}

pub fn visit_returning<'stmt, V>(v: &mut V, node: &Returning<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        Returning::Star => {}
        Returning::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn visit_stmt<'stmt, V>(v: &mut V, node: &Statement<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        Statement::Delete(stmt) => v.visit_stmt_delete(stmt),
        Statement::Link(stmt) => v.visit_stmt_link(stmt),
        Statement::Insert(stmt) => v.visit_stmt_insert(stmt),
        Statement::Query(stmt) => v.visit_stmt_query(stmt),
        Statement::Unlink(stmt) => v.visit_stmt_unlink(stmt),
        Statement::Update(stmt) => v.visit_stmt_update(stmt),
    }
}

pub fn visit_stmt_delete<'stmt, V>(v: &mut V, node: &Delete<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt_query(&node.selection);
}

pub fn visit_stmt_link<'stmt, V>(v: &mut V, node: &Link<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt_query(&node.source);
    v.visit_stmt_query(&node.target);
}

pub fn visit_stmt_insert<'stmt, V>(v: &mut V, node: &Insert<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt_query(&node.scope);
    v.visit_expr(&node.values);

    if let Some(returning) = &node.returning {
        v.visit_returning(returning);
    }
}

pub fn visit_stmt_query<'stmt, V>(v: &mut V, node: &Query<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr_set(&node.body);
}

pub fn visit_stmt_select<'stmt, V>(v: &mut V, node: &Select<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_expr(&node.filter);
    v.visit_returning(&node.returning);
}

pub fn visit_stmt_unlink<'stmt, V>(v: &mut V, node: &Unlink<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt_query(&node.source);
    v.visit_stmt_query(&node.target);
}

pub fn visit_stmt_update<'stmt, V>(v: &mut V, node: &Update<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    v.visit_stmt_query(&node.selection);
}

pub fn visit_value<'stmt, V>(v: &mut V, node: &Value<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    match node {
        Value::Record(node) => v.visit_value_record(node),
        _ => {}
    }
}

pub fn visit_values<'stmt, V>(v: &mut V, node: &Values<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for row in &node.rows {
        for item in row {
            v.visit_expr(item);
        }
    }
}

pub fn visit_value_record<'stmt, V>(v: &mut V, node: &Record<'stmt>)
where
    V: Visit<'stmt> + ?Sized,
{
    for value in node.iter() {
        v.visit_value(value);
    }
}

pub fn for_each_expr<'stmt, F>(node: &impl Node<'stmt>, f: F)
where
    F: FnMut(&Expr<'stmt>),
{
    struct ForEach<F> {
        f: F,
    }

    impl<'stmt, F> Visit<'stmt> for ForEach<F>
    where
        F: FnMut(&Expr<'stmt>),
    {
        fn visit_expr(&mut self, node: &Expr<'stmt>) {
            visit_expr(self, node);
            (self.f)(node);
        }
    }

    node.visit(ForEach { f });
}
