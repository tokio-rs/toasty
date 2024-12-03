#![allow(unused_variables)]

use super::*;

pub trait Visit: Sized {
    fn visit<N: Node>(&mut self, i: &N) {
        i.visit(self);
    }

    fn visit_assignments(&mut self, i: &Assignments) {
        visit_assignments(self, i);
    }

    fn visit_expr(&mut self, i: &Expr) {
        visit_expr(self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd) {
        visit_expr_and(self, i);
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

    fn visit_expr_enum(&mut self, i: &ExprEnum) {
        visit_expr_enum(self, i);
    }

    fn visit_expr_field(&mut self, i: &ExprField) {
        visit_expr_field(self, i);
    }

    fn visit_expr_in_list(&mut self, i: &ExprInList) {
        visit_expr_in_list(self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery) {
        visit_expr_in_subquery(self, i);
    }

    fn visit_expr_like(&mut self, i: &ExprLike) {
        visit_expr_like(self, i);
    }

    fn visit_expr_key(&mut self, i: &ExprKey) {
        visit_expr_key(self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr) {
        visit_expr_or(self, i);
    }

    fn visit_expr_list(&mut self, i: &Vec<Expr>) {
        visit_expr_list(self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord) {
        visit_expr_record(self, i);
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

    fn visit_expr_project(&mut self, i: &ExprProject) {
        visit_expr_project(self, i);
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

    fn visit_stmt(&mut self, i: &Statement) {
        visit_stmt(self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete) {
        visit_stmt_delete(self, i);
    }

    fn visit_stmt_link(&mut self, i: &Link) {
        visit_stmt_link(self, i);
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

    fn visit_stmt_unlink(&mut self, i: &Unlink) {
        visit_stmt_unlink(self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update) {
        visit_stmt_update(self, i);
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
}

impl<'stmt, V: Visit> Visit for &mut V {
    fn visit_expr(&mut self, i: &Expr) {
        Visit::visit_expr(&mut **self, i);
    }

    fn visit_expr_and(&mut self, i: &ExprAnd) {
        Visit::visit_expr_and(&mut **self, i);
    }

    fn visit_expr_arg(&mut self, i: &ExprArg) {
        Visit::visit_expr_arg(&mut **self, i);
    }

    fn visit_expr_binary_op(&mut self, i: &ExprBinaryOp) {
        Visit::visit_expr_binary_op(&mut **self, i);
    }

    fn visit_expr_concat(&mut self, i: &ExprConcat) {
        Visit::visit_expr_concat(&mut **self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &ExprInSubquery) {
        Visit::visit_expr_in_subquery(&mut **self, i);
    }

    fn visit_expr_or(&mut self, i: &ExprOr) {
        Visit::visit_expr_or(&mut **self, i);
    }

    fn visit_expr_record(&mut self, i: &ExprRecord) {
        Visit::visit_expr_record(&mut **self, i);
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

    fn visit_stmt(&mut self, i: &Statement) {
        Visit::visit_stmt(&mut **self, i);
    }

    fn visit_stmt_query(&mut self, i: &Query) {
        Visit::visit_stmt_query(&mut **self, i);
    }

    fn visit_stmt_insert(&mut self, i: &Insert) {
        Visit::visit_stmt_insert(&mut **self, i);
    }

    fn visit_stmt_update(&mut self, i: &Update) {
        Visit::visit_stmt_update(&mut **self, i);
    }

    fn visit_stmt_delete(&mut self, i: &Delete) {
        Visit::visit_stmt_delete(&mut **self, i);
    }

    fn visit_value(&mut self, i: &Value) {
        Visit::visit_value(&mut **self, i);
    }

    fn visit_values(&mut self, i: &Values) {
        Visit::visit_values(&mut **self, i);
    }
}

pub fn visit_assignments<'stmt, V>(v: &mut V, node: &Assignments)
where
    V: Visit + ?Sized,
{
    for expr in &node.exprs {
        if let Some(expr) = expr {
            v.visit_expr(expr);
        }
    }
}

pub fn visit_expr<'stmt, V>(v: &mut V, node: &Expr)
where
    V: Visit + ?Sized,
{
    match node {
        Expr::And(expr) => v.visit_expr_and(expr),
        Expr::Arg(expr) => v.visit_expr_arg(expr),
        Expr::BinaryOp(expr) => v.visit_expr_binary_op(expr),
        Expr::Cast(expr) => v.visit_expr_cast(expr),
        Expr::Column(expr) => v.visit_expr_column(expr),
        Expr::Concat(expr) => v.visit_expr_concat(expr),
        Expr::Enum(expr) => v.visit_expr_enum(expr),
        Expr::Field(expr) => v.visit_expr_field(expr),
        Expr::InList(expr) => v.visit_expr_in_list(expr),
        Expr::InSubquery(expr) => v.visit_expr_in_subquery(expr),
        Expr::Key(expr) => v.visit_expr_key(expr),
        Expr::Or(expr) => v.visit_expr_or(expr),
        Expr::Pattern(expr) => v.visit_expr_pattern(expr),
        Expr::Project(expr) => v.visit_expr_project(expr),
        Expr::Record(expr) => v.visit_expr_record(expr),
        Expr::List(expr) => v.visit_expr_list(expr),
        Expr::Stmt(expr) => v.visit_expr_stmt(expr),
        Expr::Type(expr) => v.visit_expr_ty(expr),
        Expr::Value(expr) => v.visit_value(expr),
        _ => todo!(),
    }
}

pub fn visit_expr_and<'stmt, V>(v: &mut V, node: &ExprAnd)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_arg<'stmt, V>(v: &mut V, node: &ExprArg)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_begins_with<'stmt, V>(v: &mut V, node: &ExprBeginsWith)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_binary_op<'stmt, V>(v: &mut V, node: &ExprBinaryOp)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.lhs);
    v.visit_expr(&node.rhs);
}

pub fn visit_expr_cast<'stmt, V>(v: &mut V, node: &ExprCast)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
}

pub fn visit_expr_column<'stmt, V>(v: &mut V, node: &ExprColumn)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_concat<'stmt, V>(v: &mut V, node: &ExprConcat)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_enum<'stmt, V>(v: &mut V, node: &ExprEnum)
where
    V: Visit + ?Sized,
{
    v.visit_expr_record(&node.fields);
}

pub fn visit_expr_field<'stmt, V>(_v: &mut V, _node: &ExprField)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_in_list<'stmt, V>(v: &mut V, node: &ExprInList)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.list);
}

pub fn visit_expr_in_subquery<'stmt, V>(v: &mut V, node: &ExprInSubquery)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_stmt_query(&node.query);
}

pub fn visit_expr_like<'stmt, V>(v: &mut V, node: &ExprLike)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.expr);
    v.visit_expr(&node.pattern);
}

pub fn visit_expr_key<'stmt, V>(v: &mut V, node: &ExprKey)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_or<'stmt, V>(v: &mut V, node: &ExprOr)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_list<'stmt, V>(v: &mut V, node: &Vec<Expr>)
where
    V: Visit + ?Sized,
{
    for expr in node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_record<'stmt, V>(v: &mut V, node: &ExprRecord)
where
    V: Visit + ?Sized,
{
    for expr in &**node {
        v.visit_expr(expr);
    }
}

pub fn visit_expr_set<'stmt, V>(v: &mut V, node: &ExprSet)
where
    V: Visit + ?Sized,
{
    match node {
        ExprSet::Select(expr) => v.visit_stmt_select(expr),
        ExprSet::SetOp(expr) => v.visit_expr_set_op(expr),
        ExprSet::Values(expr) => v.visit_values(expr),
    }
}

pub fn visit_expr_set_op<'stmt, V>(v: &mut V, node: &ExprSetOp)
where
    V: Visit + ?Sized,
{
    for operand in &node.operands {
        v.visit_expr_set(operand);
    }
}

pub fn visit_expr_stmt<'stmt, V>(v: &mut V, node: &ExprStmt)
where
    V: Visit + ?Sized,
{
    v.visit_stmt(&node.stmt);
}

pub fn visit_expr_ty<'stmt, V>(v: &mut V, node: &ExprTy)
where
    V: Visit + ?Sized,
{
}

pub fn visit_expr_pattern<'stmt, V>(v: &mut V, node: &ExprPattern)
where
    V: Visit + ?Sized,
{
    match node {
        ExprPattern::BeginsWith(expr) => v.visit_expr_begins_with(expr),
        ExprPattern::Like(expr) => v.visit_expr_like(expr),
    }
}

pub fn visit_expr_project<'stmt, V>(v: &mut V, node: &ExprProject)
where
    V: Visit + ?Sized,
{
    v.visit_expr(&node.base);
    v.visit_projection(&node.projection);
}

pub fn visit_projection<'stmt, V>(v: &mut V, node: &Projection)
where
    V: Visit + ?Sized,
{
}

pub fn visit_returning<'stmt, V>(v: &mut V, node: &Returning)
where
    V: Visit + ?Sized,
{
    match node {
        Returning::Star | Returning::Changed => {}
        Returning::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn visit_source<'stmt, V>(_v: &mut V, _node: &Source)
where
    V: Visit + ?Sized,
{
}

pub fn visit_stmt<'stmt, V>(v: &mut V, node: &Statement)
where
    V: Visit + ?Sized,
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

pub fn visit_stmt_delete<'stmt, V>(v: &mut V, node: &Delete)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.from);
    v.visit_expr(&node.filter);

    if let Some(returning) = &node.returning {
        v.visit_returning(returning);
    }
}

pub fn visit_stmt_link<'stmt, V>(v: &mut V, node: &Link)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.source);
    v.visit_stmt_query(&node.target);
}

pub fn visit_stmt_insert<'stmt, V>(v: &mut V, node: &Insert)
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

pub fn visit_stmt_query<'stmt, V>(v: &mut V, node: &Query)
where
    V: Visit + ?Sized,
{
    v.visit_expr_set(&node.body);
}

pub fn visit_stmt_select<'stmt, V>(v: &mut V, node: &Select)
where
    V: Visit + ?Sized,
{
    v.visit_source(&node.source);
    v.visit_expr(&node.filter);
    v.visit_returning(&node.returning);
}

pub fn visit_stmt_unlink<'stmt, V>(v: &mut V, node: &Unlink)
where
    V: Visit + ?Sized,
{
    v.visit_stmt_query(&node.source);
    v.visit_stmt_query(&node.target);
}

pub fn visit_stmt_update<'stmt, V>(v: &mut V, node: &Update)
where
    V: Visit + ?Sized,
{
    v.visit_assignments(&node.assignments);

    if let Some(expr) = &node.filter {
        v.visit_expr(expr);
    }

    if let Some(expr) = &node.condition {
        v.visit_expr(expr);
    }
}

pub fn visit_value<'stmt, V>(v: &mut V, node: &Value)
where
    V: Visit + ?Sized,
{
    match node {
        Value::Record(node) => v.visit_value_record(node),
        _ => {}
    }
}

pub fn visit_values<'stmt, V>(v: &mut V, node: &Values)
where
    V: Visit + ?Sized,
{
    for expr in &node.rows {
        v.visit_expr(expr);
    }
}

pub fn visit_value_record<'stmt, V>(v: &mut V, node: &ValueRecord)
where
    V: Visit + ?Sized,
{
    for value in node.iter() {
        v.visit_value(value);
    }
}

pub fn for_each_expr<'stmt, F>(node: &impl Node, f: F)
where
    F: FnMut(&Expr),
{
    struct ForEach<F> {
        f: F,
    }

    impl<'stmt, F> Visit for ForEach<F>
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
