#![allow(unused_variables)]

use super::*;

pub trait Map: Sized {
    fn map<N: Node>(&mut self, i: &N) -> N {
        i.map(self)
    }

    fn map_expr(&mut self, i: &Expr) -> Expr {
        map_expr(self, i)
    }

    fn map_expr_and(&mut self, i: &ExprAnd) -> ExprAnd {
        map_expr_and(self, i)
    }

    fn map_expr_arg(&mut self, i: &ExprArg) -> ExprArg {
        map_expr_arg(self, i)
    }

    fn map_expr_binary_op(&mut self, i: &ExprBinaryOp) -> ExprBinaryOp {
        map_expr_binary_op(self, i)
    }

    fn map_expr_cast(&mut self, i: &ExprCast) -> ExprCast {
        map_expr_cast(self, i)
    }

    fn map_expr_column(&mut self, i: &ExprColumn) -> ExprColumn {
        map_expr_column(self, i)
    }

    fn map_expr_concat(&mut self, i: &ExprConcat) -> ExprConcat {
        map_expr_concat(self, i)
    }

    fn map_expr_enum(&mut self, i: &ExprEnum) -> ExprEnum {
        map_expr_enum(self, i)
    }

    fn map_expr_field(&mut self, i: &ExprField) -> ExprField {
        map_expr_field(self, i)
    }

    fn map_expr_in_subquery(&mut self, i: &ExprInSubquery) -> ExprInSubquery {
        map_expr_in_subquery(self, i)
    }

    fn map_expr_key(&mut self, i: &ExprKey) -> ExprKey {
        map_expr_key(self, i)
    }

    fn map_expr_or(&mut self, i: &ExprOr) -> ExprOr {
        map_expr_or(self, i)
    }

    fn map_expr_path(&mut self, i: &Path) -> Path {
        map_expr_path(self, i)
    }

    fn map_expr_project(&mut self, i: &ExprProject) -> ExprProject {
        map_expr_project(self, i)
    }

    fn map_expr_record(&mut self, i: &ExprRecord) -> ExprRecord {
        map_expr_record(self, i)
    }

    fn map_expr_list(&mut self, i: &Vec<Expr>) -> Vec<Expr> {
        map_expr_list(self, i)
    }

    fn map_expr_set(&mut self, i: &ExprSet) -> ExprSet {
        map_expr_set(self, i)
    }

    fn map_expr_set_op(&mut self, i: &ExprSetOp) -> ExprSetOp {
        map_expr_set_op(self, i)
    }

    fn map_expr_stmt(&mut self, i: &ExprStmt) -> ExprStmt {
        map_expr_stmt(self, i)
    }

    fn map_expr_ty(&mut self, i: &ExprTy) -> ExprTy {
        map_expr_ty(self, i)
    }

    fn map_insert_target(&mut self, i: &InsertTarget) -> InsertTarget {
        map_insert_target(self, i)
    }

    fn map_projection(&mut self, i: &Projection) -> Projection {
        map_projection(self, i)
    }

    fn map_returning(&mut self, i: &Returning) -> Returning {
        map_returning(self, i)
    }

    fn map_source(&mut self, i: &Source) -> Source {
        map_source(self, i)
    }

    fn map_stmt(&mut self, i: &Statement) -> Statement {
        map_stmt(self, i)
    }

    fn map_stmt_select(&mut self, i: &Select) -> Select {
        map_stmt_select(self, i)
    }

    fn map_stmt_insert(&mut self, i: &Insert) -> Insert {
        map_stmt_insert(self, i)
    }

    fn map_stmt_query(&mut self, i: &Query) -> Query {
        map_stmt_query(self, i)
    }

    fn map_stmt_update(&mut self, i: &Update) -> Update {
        map_stmt_update(self, i)
    }

    fn map_stmt_delete(&mut self, i: &Delete) -> Delete {
        map_stmt_delete(self, i)
    }

    fn map_stmt_link(&mut self, i: &Link) -> Link {
        map_stmt_link(self, i)
    }

    fn map_stmt_unlink(&mut self, i: &Unlink) -> Unlink {
        map_stmt_unlink(self, i)
    }

    fn map_value(&mut self, i: &Value) -> Value {
        map_value(self, i)
    }
}

pub fn map_expr<'stmt, V>(v: &mut V, node: &Expr) -> Expr
where
    V: Map + ?Sized,
{
    match node {
        Expr::And(expr) => v.map_expr_and(expr).into(),
        Expr::Arg(expr) => v.map_expr_arg(expr).into(),
        Expr::BinaryOp(expr) => v.map_expr_binary_op(expr).into(),
        Expr::Cast(expr) => v.map_expr_cast(expr).into(),
        Expr::Column(expr) => v.map_expr_column(expr).into(),
        Expr::Concat(expr) => v.map_expr_concat(expr).into(),
        Expr::Enum(expr) => v.map_expr_enum(expr).into(),
        Expr::Field(expr) => v.map_expr_field(expr).into(),
        Expr::InSubquery(expr) => v.map_expr_in_subquery(expr).into(),
        Expr::Key(expr) => v.map_expr_key(expr).into(),
        Expr::Or(expr) => v.map_expr_or(expr).into(),
        Expr::Project(expr) => v.map_expr_project(expr).into(),
        Expr::Record(expr) => v.map_expr_record(expr).into(),
        Expr::List(expr) => Expr::List(v.map_expr_list(expr)),
        Expr::Stmt(expr) => v.map_expr_stmt(expr).into(),
        Expr::Type(expr) => v.map_expr_ty(expr).into(),
        Expr::Value(expr) => v.map_value(expr).into(),
        _ => todo!(),
    }
}

pub fn map_expr_and<'stmt, V>(v: &mut V, node: &ExprAnd) -> ExprAnd
where
    V: Map + ?Sized,
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
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_binary_op<'stmt, V>(v: &mut V, node: &ExprBinaryOp) -> ExprBinaryOp
where
    V: Map + ?Sized,
{
    ExprBinaryOp {
        lhs: v.map_expr(&node.lhs).into(),
        rhs: v.map_expr(&node.rhs).into(),
        op: node.op,
    }
}

pub fn map_expr_cast<'stmt, V>(v: &mut V, node: &ExprCast) -> ExprCast
where
    V: Map + ?Sized,
{
    ExprCast {
        expr: Box::new(v.map_expr(&*node.expr)),
        ty: node.ty.clone(),
    }
}

pub fn map_expr_column<'stmt, V>(v: &mut V, node: &ExprColumn) -> ExprColumn
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_concat<'stmt, V>(v: &mut V, node: &ExprConcat) -> ExprConcat
where
    V: Map + ?Sized,
{
    ExprConcat {
        exprs: node.exprs.iter().map(|expr| v.map_expr(expr)).collect(),
    }
}

pub fn map_expr_enum<'stmt, V>(v: &mut V, node: &ExprEnum) -> ExprEnum
where
    V: Map + ?Sized,
{
    ExprEnum {
        variant: node.variant,
        fields: v.map_expr_record(&node.fields).into(),
    }
}

pub fn map_expr_field<'stmt, V>(v: &mut V, node: &ExprField) -> ExprField
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_in_subquery<'stmt, V>(v: &mut V, node: &ExprInSubquery) -> ExprInSubquery
where
    V: Map + ?Sized,
{
    ExprInSubquery {
        expr: v.map_expr(&node.expr).into(),
        query: v.map_stmt_query(&node.query).into(),
    }
}

pub fn map_expr_key<'stmt, V>(v: &mut V, node: &ExprKey) -> ExprKey
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_or<'stmt, V>(v: &mut V, node: &ExprOr) -> ExprOr
where
    V: Map + ?Sized,
{
    ExprOr {
        operands: node
            .operands
            .iter()
            .map(|operand| v.map_expr(operand))
            .collect(),
    }
}

pub fn map_expr_record<'stmt, V>(v: &mut V, node: &ExprRecord) -> ExprRecord
where
    V: Map + ?Sized,
{
    ExprRecord::from_vec(node.iter().map(|expr| v.map_expr(expr)).collect())
}

pub fn map_expr_list<'stmt, V>(v: &mut V, node: &Vec<Expr>) -> Vec<Expr>
where
    V: Map + ?Sized,
{
    todo!()
}

pub fn map_expr_set<'stmt, V>(v: &mut V, node: &ExprSet) -> ExprSet
where
    V: Map + ?Sized,
{
    match node {
        ExprSet::Select(expr) => ExprSet::Select(v.map_stmt_select(expr)),
        ExprSet::SetOp(expr) => ExprSet::SetOp(v.map_expr_set_op(expr)),
        ExprSet::Values(_) => todo!(),
    }
}

pub fn map_expr_set_op<'stmt, V>(v: &mut V, node: &ExprSetOp) -> ExprSetOp
where
    V: Map + ?Sized,
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

pub fn map_expr_stmt<'stmt, V>(v: &mut V, node: &ExprStmt) -> ExprStmt
where
    V: Map + ?Sized,
{
    v.map_stmt(&node.stmt).into()
}

pub fn map_expr_ty<'stmt, V>(v: &mut V, node: &ExprTy) -> ExprTy
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_path<'stmt, V>(v: &mut V, node: &Path) -> Path
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_expr_project<'stmt, V>(v: &mut V, node: &ExprProject) -> ExprProject
where
    V: Map + ?Sized,
{
    ExprProject {
        base: Box::new(v.map_expr(&node.base)),
        projection: v.map_projection(&node.projection),
    }
}

pub fn map_insert_target<'stmt, V>(v: &mut V, node: &InsertTarget) -> InsertTarget
where
    V: Map + ?Sized,
{
    match node {
        InsertTarget::Scope(query) => InsertTarget::Scope(v.map_stmt_query(query)),
        _ => node.clone(),
    }
}

pub fn map_projection<'stmt, V>(v: &mut V, node: &Projection) -> Projection
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_returning<'stmt, V>(v: &mut V, node: &Returning) -> Returning
where
    V: Map + ?Sized,
{
    match node {
        Returning::Star | Returning::Changed => node.clone(),
        Returning::Expr(expr) => Returning::Expr(v.map_expr(expr)),
    }
}

pub fn map_source<'stmt, V>(v: &mut V, node: &Source) -> Source
where
    V: Map + ?Sized,
{
    node.clone()
}

pub fn map_stmt<'stmt, V>(v: &mut V, node: &Statement) -> Statement
where
    V: Map + ?Sized,
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

pub fn map_stmt_select<'stmt, V>(v: &mut V, node: &Select) -> Select
where
    V: Map + ?Sized,
{
    Select {
        source: v.map_source(&node.source),
        filter: v.map_expr(&node.filter),
        returning: v.map_returning(&node.returning),
    }
}

pub fn map_stmt_insert<'stmt, V>(v: &mut V, node: &Insert) -> Insert
where
    V: Map + ?Sized,
{
    Insert {
        target: v.map_insert_target(&node.target),
        source: v.map_stmt_query(&node.source),
        returning: node
            .returning
            .as_ref()
            .map(|returning| v.map_returning(returning)),
    }
}

pub fn map_stmt_query<'stmt, V>(v: &mut V, node: &Query) -> Query
where
    V: Map + ?Sized,
{
    Query {
        body: Box::new(v.map_expr_set(&node.body)),
    }
}

pub fn map_stmt_update<'stmt, V>(v: &mut V, node: &Update) -> Update
where
    V: Map + ?Sized,
{
    todo!()
}

pub fn map_stmt_delete<'stmt, V>(v: &mut V, node: &Delete) -> Delete
where
    V: Map + ?Sized,
{
    Delete {
        from: v.map_source(&node.from),
        filter: v.map_expr(&node.filter),
        returning: node
            .returning
            .as_ref()
            .map(|returning| v.map_returning(returning)),
    }
}

pub fn map_stmt_link<'stmt, V>(v: &mut V, node: &Link) -> Link
where
    V: Map + ?Sized,
{
    Link {
        field: node.field,
        source: v.map_stmt_query(&node.source),
        target: v.map_stmt_query(&node.target),
    }
}

pub fn map_stmt_unlink<'stmt, V>(v: &mut V, node: &Unlink) -> Unlink
where
    V: Map + ?Sized,
{
    Unlink {
        field: node.field,
        source: v.map_stmt_query(&node.source),
        target: v.map_stmt_query(&node.target),
    }
}

pub fn map_value<'stmt, V>(v: &mut V, node: &Value) -> Value
where
    V: Map + ?Sized,
{
    node.clone()
}
