use super::*;

use std::ops;

#[derive(Clone, PartialEq)]
pub enum Expr {
    /// AND a set of binary expressions
    And(ExprAnd),

    /// An argument when the expression is a function body
    Arg(ExprArg),

    /// Binary expression
    BinaryOp(ExprBinaryOp),

    /// Cast an expression to a different type
    Cast(ExprCast),

    /// References a column from a table in the statement
    Column(ExprColumn),

    /// Concat multiple expressions together
    /// TODO: name this something different?
    Concat(ExprConcat),

    /// Concat strings
    ConcatStr(ExprConcatStr),

    /// Return an enum value
    Enum(ExprEnum),

    /// References a field in the statement
    Field(ExprField),

    /// In list
    InList(ExprInList),

    /// The expression is contained by the given subquery
    InSubquery(ExprInSubquery),

    /// Whether an expression is (or is not) null. This is different from a
    /// binary expression because of how databases treat null comparisons.
    IsNull(ExprIsNull),

    /// References a model's primary key
    Key(ExprKey),

    /// Apply an expression to each item in a list
    Map(ExprMap),

    /// OR a set of binary expressi5nos
    Or(ExprOr),

    /// Checks if an expression matches a pattern.
    Pattern(ExprPattern),

    /// Project an expression
    Project(ExprProject),

    /// Evaluates to a tuple value
    Record(ExprRecord),

    /// A list of expressions of the same type
    List(Vec<Expr>),

    /// Evaluate a sub-statement
    Stmt(ExprStmt),

    /// A type reference. This is used by the "is a" expression
    Type(ExprTy),

    /// Evaluates to a constant value reference
    Value(Value),

    // TODO: get rid of this?
    DecodeEnum(Box<Expr>, Type, usize),
}

impl Expr {
    pub fn null() -> Expr {
        Expr::Value(Value::Null)
    }

    /// Is a value that evaluates to null
    pub fn is_value_null(&self) -> bool {
        matches!(self, Expr::Value(Value::Null))
    }

    pub fn list<T>(items: impl IntoIterator<Item = T>) -> Expr
    where
        T: Into<Expr>,
    {
        Expr::List(items.into_iter().map(Into::into).collect())
    }

    /// Returns true if the expression is the `true` boolean expression
    pub fn is_true(&self) -> bool {
        matches!(self, Expr::Value(Value::Bool(true)))
    }

    /// Returns true if the expression is a constant value.
    pub fn is_value(&self) -> bool {
        matches!(self, Expr::Value(..))
    }

    pub fn is_stmt(&self) -> bool {
        matches!(self, Expr::Stmt(..))
    }

    /// Returns true if the expression is a binary operation
    pub fn is_binary_op(&self) -> bool {
        matches!(self, Expr::BinaryOp(..))
    }

    pub fn is_arg(&self) -> bool {
        matches!(self, Expr::Arg(_))
    }

    pub fn into_value(self) -> Value {
        match self {
            Expr::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn into_stmt(self) -> ExprStmt {
        match self {
            Expr::Stmt(stmt) => stmt,
            _ => todo!(),
        }
    }

    /// Returns `true` if the expression is a constant expression.
    pub fn is_const(&self) -> bool {
        match self {
            Expr::Value(_) => true,
            Expr::Record(expr_record) => expr_record.iter().all(|expr| expr.is_const()),
            _ => false,
        }
    }

    pub fn map_projections(&self, f: impl FnMut(&Projection) -> Projection) -> Expr {
        struct MapProjections<T>(T);

        impl<T: FnMut(&Projection) -> Projection> VisitMut for MapProjections<T> {
            fn visit_projection_mut(&mut self, i: &mut Projection) {
                *i = self.0(i);
            }
        }

        let mut mapped = self.clone();
        MapProjections(f).visit_expr_mut(&mut mapped);
        mapped
    }

    #[track_caller]
    pub fn entry(&self, path: impl EntryPath) -> Entry<'_> {
        let mut ret = Entry::Expr(self);

        for step in path.step_iter() {
            ret = match ret {
                Entry::Expr(Expr::Record(expr)) => Entry::Expr(&expr[step.into_usize()]),
                Entry::Value(Value::Record(record))
                | Entry::Expr(Expr::Value(Value::Record(record))) => {
                    Entry::Value(&record[step.into_usize()])
                }
                _ => todo!("ret={ret:#?}; base={self:#?}; step={step:#?}"),
            }
        }

        ret
    }

    #[track_caller]
    pub fn entry_mut(&mut self, path: impl EntryPath) -> EntryMut<'_> {
        let mut ret = EntryMut::Expr(self);

        for step in path.step_iter() {
            ret = match ret {
                EntryMut::Expr(Expr::Record(expr)) => EntryMut::Expr(&mut expr[step.into_usize()]),
                EntryMut::Value(Value::Record(record))
                | EntryMut::Expr(Expr::Value(Value::Record(record))) => {
                    EntryMut::Value(&mut record[step.into_usize()])
                }
                _ => todo!("ret={ret:#?}; step={step:#?}"),
            }
        }

        ret
    }

    /// Assume the expression evaluates to a set of records and extend the
    /// evaluation to include the given expression.
    ///
    /// TODO: maybe split up the ops and instead have `expr.as_or_cast_to_concat().push()`
    pub fn push(&mut self, expr: impl Into<Expr>) {
        use std::mem;

        match self {
            Expr::Concat(exprs) => exprs.push(expr.into()),
            Expr::Value(Value::Null) => {
                *self = ExprConcat::new(vec![expr.into()]).into();
            }
            _ => {
                let prev = mem::replace(self, Expr::Value(Value::Null));
                *self = ExprConcat::new(vec![prev, expr.into()]).into();
            }
        }
    }

    pub fn take(&mut self) -> Expr {
        std::mem::replace(self, Expr::Value(Value::Null))
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        visit_mut::for_each_expr_mut(self, move |expr| match expr {
            Expr::Arg(expr_arg) => {
                *expr = input.resolve_arg(expr_arg);
            }
            Expr::Map(_) => todo!(),
            _ => {}
        });
    }
}

impl Default for Expr {
    fn default() -> Self {
        Expr::Value(Value::default())
    }
}

impl Node for Expr {
    fn map<V: Map>(&self, visit: &mut V) -> Self {
        visit.map_expr(self)
    }

    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_expr(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_expr_mut(self);
    }
}

// === Conversions ===

impl From<bool> for Expr {
    fn from(value: bool) -> Expr {
        Expr::Value(Value::from(value))
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Self {
        Expr::Value(value.into())
    }
}

impl From<&i64> for Expr {
    fn from(value: &i64) -> Self {
        Expr::Value(value.into())
    }
}

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Expr::Value(value.into())
    }
}

impl From<&String> for Expr {
    fn from(value: &String) -> Self {
        Expr::Value(value.into())
    }
}

impl From<&str> for Expr {
    fn from(value: &str) -> Self {
        Expr::Value(value.into())
    }
}

impl From<Value> for Expr {
    fn from(value: Value) -> Expr {
        Expr::Value(value)
    }
}

impl<E1, E2> From<(E1, E2)> for Expr
where
    E1: Into<Expr>,
    E2: Into<Expr>,
{
    fn from(value: (E1, E2)) -> Expr {
        Expr::Record(value.into())
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::And(e) => e.fmt(f),
            Expr::Arg(e) => e.fmt(f),
            Expr::BinaryOp(e) => e.fmt(f),
            Expr::Cast(e) => e.fmt(f),
            Expr::Column(e) => e.fmt(f),
            Expr::Concat(e) => e.fmt(f),
            Expr::ConcatStr(e) => e.fmt(f),
            Expr::Enum(e) => e.fmt(f),
            Expr::Field(e) => e.fmt(f),
            Expr::InList(e) => e.fmt(f),
            Expr::InSubquery(e) => e.fmt(f),
            Expr::IsNull(e) => e.fmt(f),
            Expr::Key(e) => e.fmt(f),
            Expr::Map(e) => e.fmt(f),
            Expr::Or(e) => e.fmt(f),
            Expr::Pattern(e) => e.fmt(f),
            Expr::Project(e) => e.fmt(f),
            Expr::Record(e) => e.fmt(f),
            Expr::List(e) => e.fmt(f),
            Expr::Stmt(e) => e.fmt(f),
            Expr::Type(e) => e.fmt(f),
            Expr::Value(e) => e.fmt(f),
            Expr::DecodeEnum(expr, ty, variant) => f
                .debug_tuple("DecodeEnum")
                .field(expr)
                .field(ty)
                .field(variant)
                .finish(),
        }
    }
}
