#![allow(unused_variables)]

use super::*;

pub trait Map<'stmt>: Sized {
    fn map<N: Node<'stmt>>(&mut self, i: &N) -> N {
        i.map(self)
    }

    fn map_expr(&mut self, i: &Expr<'stmt>) -> Expr<'stmt> {
        map_expr(self, i)
    }

    fn map_expr_and(&mut self, i: &ExprAnd<'stmt>) -> ExprAnd<'stmt> {
        map_expr_and(self, i)
    }

    fn map_expr_arg(&mut self, i: &ExprArg) -> ExprArg {
        map_expr_arg(self, i)
    }

    fn map_expr_binary_op(&mut self, i: &ExprBinaryOp<'stmt>) -> ExprBinaryOp<'stmt> {
        map_expr_binary_op(self, i)
    }

    fn map_expr_concat(&mut self, i: &ExprConcat<'stmt>) -> ExprConcat<'stmt> {
        map_expr_concat(self, i)
    }

    fn map_expr_enum(&mut self, i: &ExprEnum<'stmt>) -> ExprEnum<'stmt> {
        map_expr_enum(self, i)
    }

    fn map_expr_in_subquery(&mut self, i: &ExprInSubquery<'stmt>) -> ExprInSubquery<'stmt> {
        map_expr_in_subquery(self, i)
    }

    fn map_expr_or(&mut self, i: &ExprOr<'stmt>) -> ExprOr<'stmt> {
        map_expr_or(self, i)
    }

    fn map_expr_path(&mut self, i: &Path) -> Path {
        map_expr_path(self, i)
    }

    fn map_expr_project(&mut self, i: &ExprProject<'stmt>) -> ExprProject<'stmt> {
        map_expr_project(self, i)
    }

    fn map_expr_record(&mut self, i: &ExprRecord<'stmt>) -> ExprRecord<'stmt> {
        map_expr_record(self, i)
    }

    fn map_expr_list(&mut self, i: &Vec<Expr<'stmt>>) -> Vec<Expr<'stmt>> {
        map_expr_list(self, i)
    }

    fn map_expr_set(&mut self, i: &ExprSet<'stmt>) -> ExprSet<'stmt> {
        map_expr_set(self, i)
    }

    fn map_expr_set_op(&mut self, i: &ExprSetOp<'stmt>) -> ExprSetOp<'stmt> {
        map_expr_set_op(self, i)
    }

    fn map_expr_stmt(&mut self, i: &ExprStmt<'stmt>) -> ExprStmt<'stmt> {
        map_expr_stmt(self, i)
    }

    fn map_expr_ty(&mut self, i: &ExprTy) -> ExprTy {
        map_expr_ty(self, i)
    }

    fn map_project_base(&mut self, i: &ProjectBase<'stmt>) -> ProjectBase<'stmt> {
        map_project_base(self, i)
    }

    fn map_projection(&mut self, i: &Projection) -> Projection {
        map_projection(self, i)
    }

    fn map_returning(&mut self, i: &Returning<'stmt>) -> Returning<'stmt> {
        map_returning(self, i)
    }

    fn map_stmt(&mut self, i: &Statement<'stmt>) -> Statement<'stmt> {
        map_stmt(self, i)
    }

    fn map_stmt_select(&mut self, i: &Select<'stmt>) -> Select<'stmt> {
        map_stmt_select(self, i)
    }

    fn map_stmt_insert(&mut self, i: &Insert<'stmt>) -> Insert<'stmt> {
        map_stmt_insert(self, i)
    }

    fn map_stmt_query(&mut self, i: &Query<'stmt>) -> Query<'stmt> {
        map_stmt_query(self, i)
    }

    fn map_stmt_update(&mut self, i: &Update<'stmt>) -> Update<'stmt> {
        map_stmt_update(self, i)
    }

    fn map_stmt_delete(&mut self, i: &Delete<'stmt>) -> Delete<'stmt> {
        map_stmt_delete(self, i)
    }

    fn map_stmt_link(&mut self, i: &Link<'stmt>) -> Link<'stmt> {
        map_stmt_link(self, i)
    }

    fn map_stmt_unlink(&mut self, i: &Unlink<'stmt>) -> Unlink<'stmt> {
        map_stmt_unlink(self, i)
    }

    fn map_value(&mut self, i: &Value<'stmt>) -> Value<'stmt> {
        map_value(self, i)
    }
}

pub fn map_expr<'stmt, V>(v: &mut V, node: &Expr<'stmt>) -> Expr<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    match node {
        Expr::And(expr) => v.map_expr_and(expr).into(),
        Expr::Arg(expr) => v.map_expr_arg(expr).into(),
        Expr::BinaryOp(expr) => v.map_expr_binary_op(expr).into(),
        Expr::Concat(expr) => v.map_expr_concat(expr).into(),
        Expr::Enum(expr) => v.map_expr_enum(expr).into(),
        Expr::InList(expr) => todo!(),
        Expr::InSubquery(expr) => v.map_expr_in_subquery(expr).into(),
        Expr::Or(expr) => v.map_expr_or(expr).into(),
        Expr::Pattern(_) => todo!(),
        Expr::Project(expr) => v.map_expr_project(expr).into(),
        Expr::Record(expr) => v.map_expr_record(expr).into(),
        Expr::List(expr) => Expr::List(v.map_expr_list(expr)),
        Expr::Stmt(expr) => v.map_expr_stmt(expr).into(),
        Expr::Type(expr) => v.map_expr_ty(expr).into(),
        Expr::Value(expr) => v.map_value(expr).into(),
    }
}

pub fn map_expr_and<'stmt, V>(v: &mut V, node: &ExprAnd<'stmt>) -> ExprAnd<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprAnd {
        operands: node
            .operands
            .iter()
            .map(|operand| v.map_expr(operand))
            .collect(),
    }
}

pub fn map_expr_arg<'stmt, V>(v: &mut V, node: &ExprArg) -> ExprArg
where
    V: Map<'stmt> + ?Sized,
{
    node.clone()
}

pub fn map_expr_binary_op<'stmt, V>(v: &mut V, node: &ExprBinaryOp<'stmt>) -> ExprBinaryOp<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprBinaryOp {
        lhs: v.map_expr(&node.lhs).into(),
        rhs: v.map_expr(&node.rhs).into(),
        op: node.op,
    }
}

pub fn map_expr_concat<'stmt, V>(v: &mut V, node: &ExprConcat<'stmt>) -> ExprConcat<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprConcat {
        exprs: node.exprs.iter().map(|expr| v.map_expr(expr)).collect(),
    }
}

pub fn map_expr_enum<'stmt, V>(v: &mut V, node: &ExprEnum<'stmt>) -> ExprEnum<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprEnum {
        variant: node.variant,
        fields: v.map_expr_record(&node.fields).into(),
    }
}

pub fn map_expr_in_subquery<'stmt, V>(
    v: &mut V,
    node: &ExprInSubquery<'stmt>,
) -> ExprInSubquery<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprInSubquery {
        expr: v.map_expr(&node.expr).into(),
        query: v.map_stmt_query(&node.query).into(),
    }
}

pub fn map_expr_or<'stmt, V>(v: &mut V, node: &ExprOr<'stmt>) -> ExprOr<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprOr {
        operands: node
            .operands
            .iter()
            .map(|operand| v.map_expr(operand))
            .collect(),
    }
}

pub fn map_expr_record<'stmt, V>(v: &mut V, node: &ExprRecord<'stmt>) -> ExprRecord<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprRecord::from_vec(node.iter().map(|expr| v.map_expr(expr)).collect())
}

pub fn map_expr_list<'stmt, V>(v: &mut V, node: &Vec<Expr<'stmt>>) -> Vec<Expr<'stmt>>
where
    V: Map<'stmt> + ?Sized,
{
    todo!()
}

pub fn map_expr_set<'stmt, V>(v: &mut V, node: &ExprSet<'stmt>) -> ExprSet<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    match node {
        ExprSet::Select(expr) => ExprSet::Select(v.map_stmt_select(expr)),
        ExprSet::SetOp(expr) => ExprSet::SetOp(v.map_expr_set_op(expr)),
        ExprSet::Values(_) => todo!(),
    }
}

pub fn map_expr_set_op<'stmt, V>(v: &mut V, node: &ExprSetOp<'stmt>) -> ExprSetOp<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprSetOp {
        op: node.op,
        operands: node
            .operands
            .iter()
            .map(|operand| v.map_expr_set(operand))
            .collect(),
    }
}

pub fn map_expr_stmt<'stmt, V>(v: &mut V, node: &ExprStmt<'stmt>) -> ExprStmt<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    v.map_stmt(&node.stmt).into()
}

pub fn map_expr_ty<'stmt, V>(v: &mut V, node: &ExprTy) -> ExprTy
where
    V: Map<'stmt> + ?Sized,
{
    node.clone()
}

pub fn map_expr_path<'stmt, V>(v: &mut V, node: &Path) -> Path
where
    V: Map<'stmt> + ?Sized,
{
    node.clone()
}

pub fn map_expr_project<'stmt, V>(v: &mut V, node: &ExprProject<'stmt>) -> ExprProject<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    ExprProject {
        base: v.map_project_base(&node.base),
        projection: v.map_projection(&node.projection),
    }
}

pub fn map_project_base<'stmt, V>(v: &mut V, node: &ProjectBase<'stmt>) -> ProjectBase<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    match node {
        ProjectBase::ExprSelf => ProjectBase::ExprSelf,
        ProjectBase::Expr(e) => ProjectBase::Expr(Box::new(v.map_expr(e))),
    }
}

pub fn map_projection<'stmt, V>(v: &mut V, node: &Projection) -> Projection
where
    V: Map<'stmt> + ?Sized,
{
    node.clone()
}

pub fn map_returning<'stmt, V>(v: &mut V, node: &Returning<'stmt>) -> Returning<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    match node {
        Returning::Star => Returning::Star,
        Returning::Expr(expr) => Returning::Expr(v.map_expr(expr)),
    }
}

pub fn map_stmt<'stmt, V>(v: &mut V, node: &Statement<'stmt>) -> Statement<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    match node {
        Statement::Delete(stmt) => v.map_stmt_delete(stmt).into(),
        Statement::Link(_) => todo!(),
        Statement::Insert(stmt) => v.map_stmt_insert(stmt).into(),
        Statement::Query(stmt) => v.map_stmt_query(stmt).into(),
        Statement::Unlink(_) => todo!(),
        Statement::Update(stmt) => v.map_stmt_update(stmt).into(),
    }
}

pub fn map_stmt_select<'stmt, V>(v: &mut V, node: &Select<'stmt>) -> Select<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Select {
        source: node.source.clone(),
        filter: v.map_expr(&node.filter),
        returning: v.map_returning(&node.returning),
    }
}

pub fn map_stmt_insert<'stmt, V>(v: &mut V, node: &Insert<'stmt>) -> Insert<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Insert {
        scope: v.map_stmt_query(&node.scope),
        values: v.map_expr(&node.values),
        returning: node
            .returning
            .as_ref()
            .map(|returning| v.map_returning(returning)),
    }
}

pub fn map_stmt_query<'stmt, V>(v: &mut V, node: &Query<'stmt>) -> Query<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Query {
        body: Box::new(v.map_expr_set(&node.body)),
    }
}

pub fn map_stmt_update<'stmt, V>(v: &mut V, node: &Update<'stmt>) -> Update<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    todo!()
}

pub fn map_stmt_delete<'stmt, V>(v: &mut V, node: &Delete<'stmt>) -> Delete<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Delete {
        selection: v.map_stmt_query(&node.selection),
    }
}

pub fn map_stmt_link<'stmt, V>(v: &mut V, node: &Link<'stmt>) -> Link<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Link {
        field: node.field,
        source: v.map_stmt_query(&node.source),
        target: v.map_stmt_query(&node.target),
    }
}

pub fn map_stmt_unlink<'stmt, V>(v: &mut V, node: &Unlink<'stmt>) -> Unlink<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    Unlink {
        field: node.field,
        source: v.map_stmt_query(&node.source),
        target: v.map_stmt_query(&node.target),
    }
}

pub fn map_value<'stmt, V>(v: &mut V, node: &Value<'stmt>) -> Value<'stmt>
where
    V: Map<'stmt> + ?Sized,
{
    node.clone()
}
