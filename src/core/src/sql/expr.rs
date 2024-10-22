use super::*;

use crate::stmt::Value;

#[derive(Debug, Clone)]
pub enum Expr<'stmt> {
    And(ExprAnd<'stmt>),
    BeginsWith(ExprBeginsWith<'stmt>),
    BinaryOp(ExprBinaryOp<'stmt>),
    /// Unaliased column
    Column(ColumnId),
    InList(ExprInList<'stmt>),
    InSubquery(ExprInSubquery<'stmt>),
    IsNotNull(Box<Expr<'stmt>>),
    IsNull(Box<Expr<'stmt>>),
    Like(ExprLike<'stmt>),
    Or(ExprOr<'stmt>),
    Placeholder(ExprPlaceholder),
    Tuple(ExprTuple<'stmt>),
    Value(Value<'stmt>),
}

impl<'stmt> Expr<'stmt> {
    pub fn and<T>(items: impl IntoIterator<Item = T>) -> Expr<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        Expr::And(ExprAnd {
            operands: items.into_iter().map(Into::into).collect(),
        })
    }

    pub fn column(column: impl Into<ColumnId>) -> Expr<'stmt> {
        Expr::Column(column.into())
    }

    pub fn eq(lhs: impl Into<Expr<'stmt>>, rhs: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        ExprBinaryOp {
            op: BinaryOp::Eq,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn into_value(self) -> Value<'stmt> {
        match self {
            Expr::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn as_value(&self) -> &Value<'stmt> {
        match self {
            Expr::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn substitute(&mut self, mut input: impl substitute::Input<'stmt>) {
        self.substitute_ref(&mut input);
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        match self {
            Expr::Column(_) => {}
            Expr::InList(expr_in_list) => {
                if let Some(expr) = expr_in_list.substitute_ref(input) {
                    *self = expr;
                }
            }
            Expr::Placeholder(expr_placeholder) => {
                let target = input.resolve_placeholder(expr_placeholder);
                *self = target;
            }
            _ => todo!("self = {:#?}", self),
        }
    }

    pub fn from_stmt(schema: &Schema, table: TableId, stmt: stmt::Expr<'stmt>) -> Expr<'stmt> {
        match stmt {
            stmt::Expr::Arg(expr_arg) => {
                // TODO: not always true
                assert_eq!(expr_arg.position, 0);

                Expr::Placeholder(ExprPlaceholder {
                    position: expr_arg.position,
                })
            }
            stmt::Expr::And(expr_and) => Expr::And(ExprAnd {
                operands: expr_and
                    .operands
                    .into_iter()
                    .map(|stmt| Expr::from_stmt(schema, table, stmt))
                    .collect(),
            }),
            stmt::Expr::Or(expr_or) => Expr::Or(ExprOr {
                operands: expr_or
                    .operands
                    .into_iter()
                    .map(|stmt| Expr::from_stmt(schema, table, stmt))
                    .collect(),
            }),
            stmt::Expr::BinaryOp(expr_binary_op) if expr_binary_op.op.is_a() => {
                Expr::BeginsWith(ExprBeginsWith {
                    expr: Box::new(Expr::from_stmt(schema, table, *expr_binary_op.lhs)),
                    pattern: match &*expr_binary_op.rhs {
                        stmt::Expr::Type(expr_ty) => {
                            let value = expr_ty.variant.unwrap().to_string();
                            Box::new(Expr::Value(value.into()))
                        }
                        _ => todo!(),
                    },
                })
            }
            stmt::Expr::BinaryOp(expr_binary_op) => {
                if expr_binary_op.lhs.is_null() {
                    // Requires special handling
                    assert!(!expr_binary_op.rhs.is_null());

                    match expr_binary_op.op {
                        stmt::BinaryOp::Eq => Expr::IsNull(Box::new(Expr::from_stmt(
                            schema,
                            table,
                            *expr_binary_op.rhs,
                        ))),
                        stmt::BinaryOp::Ne => Expr::IsNotNull(Box::new(Expr::from_stmt(
                            schema,
                            table,
                            *expr_binary_op.rhs,
                        ))),
                        _ => todo!(),
                    }
                } else if expr_binary_op.rhs.is_null() {
                    // Requires special handling
                    assert!(!expr_binary_op.lhs.is_null());

                    match expr_binary_op.op {
                        stmt::BinaryOp::Eq => Expr::IsNull(Box::new(Expr::from_stmt(
                            schema,
                            table,
                            *expr_binary_op.lhs,
                        ))),
                        stmt::BinaryOp::Ne => Expr::IsNotNull(Box::new(Expr::from_stmt(
                            schema,
                            table,
                            *expr_binary_op.lhs,
                        ))),
                        _ => todo!(),
                    }
                } else {
                    Expr::BinaryOp(ExprBinaryOp {
                        lhs: Box::new(Expr::from_stmt(schema, table, *expr_binary_op.lhs)),
                        op: BinaryOp::from_stmt(expr_binary_op.op),
                        rhs: Box::new(Expr::from_stmt(schema, table, *expr_binary_op.rhs)),
                    })
                }
            }
            stmt::Expr::Project(expr_project) => {
                assert!(expr_project.base.is_expr_self());
                let [step] = expr_project.projection.as_slice() else {
                    todo!("expr = {:#?}", expr_project)
                };

                // TODO: not always true
                Expr::Column(ColumnId {
                    table,
                    index: step.into_usize(),
                })
            }
            stmt::Expr::Value(value) => Expr::Value(value),
            stmt::Expr::Enum(expr_enum) => {
                let fields = expr_enum.fields.iter().map(|e| e.eval_const()).collect();

                Expr::Value(stmt::Value::Enum(stmt::ValueEnum {
                    variant: expr_enum.variant,
                    fields: stmt::Record::from_vec(fields),
                }))
            }
            stmt::Expr::InList(expr_in_list) => Expr::InList(ExprInList {
                expr: Box::new(Expr::from_stmt(schema, table, *expr_in_list.expr)),
                list: Expr::list_from_stmt(schema, table, *expr_in_list.list),
            }),
            stmt::Expr::InSubquery(expr_in_subquery) => {
                let submodel =
                    schema.model(expr_in_subquery.query.body.as_select().source.as_model_id());

                let stmt::ExprSet::Select(subquery) = *expr_in_subquery.query.body else {
                    todo!()
                };
                let stmt::Returning::Expr(project) = subquery.returning else {
                    todo!()
                };

                let Statement::Query(query) =
                    Statement::query(schema, submodel.lowering.table, project, subquery.filter)
                else {
                    todo!()
                };

                Expr::InSubquery(ExprInSubquery {
                    expr: Box::new(Expr::from_stmt(schema, table, *expr_in_subquery.expr)),
                    subquery: Box::new(query),
                })
            }
            stmt::Expr::Record(expr_record) => {
                let exprs = expr_record
                    .into_iter()
                    .map(|stmt| Expr::from_stmt(schema, table, stmt))
                    .collect();
                Expr::Tuple(ExprTuple { exprs })
            }
            _ => todo!("expr = {:#?}", stmt),
        }
    }

    fn list_from_stmt(schema: &Schema, table: TableId, stmt: stmt::Expr<'stmt>) -> ExprList<'stmt> {
        match stmt {
            stmt::Expr::List(exprs) => {
                let mut ret = vec![];

                for expr in exprs {
                    ret.push(Expr::from_stmt(schema, table, expr));
                }

                ExprList::Expr(ret)
            }
            stmt::Expr::Arg(expr_arg) => ExprList::Placeholder(ExprPlaceholder {
                position: expr_arg.position,
            }),
            _ => todo!("expr={:#?}", stmt),
        }
    }
}

impl<'stmt> Default for Expr<'stmt> {
    fn default() -> Self {
        Expr::Value(Value::default())
    }
}

impl<'stmt> From<stmt::Value<'stmt>> for Expr<'stmt> {
    fn from(value: stmt::Value<'stmt>) -> Self {
        Expr::Value(value)
    }
}
