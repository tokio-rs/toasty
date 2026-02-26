use crate::stmt::{ExprExists, Input};

use super::{
    expr_reference::ExprReference, Entry, EntryMut, EntryPath, ExprAnd, ExprAny, ExprArg,
    ExprBinaryOp, ExprCast, ExprFunc, ExprInList, ExprInSubquery, ExprIsNull, ExprList, ExprMap,
    ExprNot, ExprOr, ExprProject, ExprRecord, ExprStmt, Node, Projection, Substitute, Value, Visit,
    VisitMut,
};
use std::fmt;

/// An expression.
#[derive(Clone, PartialEq)]
pub enum Expr {
    /// AND a set of binary expressions
    And(ExprAnd),

    /// ANY - returns true if any of the items evaluate to true
    Any(ExprAny),

    /// An argument when the expression is a function body
    Arg(ExprArg),

    /// Binary expression
    BinaryOp(ExprBinaryOp),

    /// Cast an expression to a different type
    Cast(ExprCast),

    /// Suggests that the database should use its default value. Useful for
    /// auto-increment fields and other columns with default values.
    Default,

    /// An exists expression `[ NOT ] EXISTS(SELECT ...)`, used in expressions like
    /// `WHERE [ NOT ] EXISTS (SELECT ...)`.
    Exists(ExprExists),

    /// Function call
    Func(ExprFunc),

    /// In list
    InList(ExprInList),

    /// The expression is contained by the given subquery
    InSubquery(ExprInSubquery),

    /// Whether an expression is (or is not) null. This is different from a
    /// binary expression because of how databases treat null comparisons.
    IsNull(ExprIsNull),

    /// Apply an expression to each item in a list
    Map(ExprMap),

    /// Negates a boolean expression
    Not(ExprNot),

    /// OR a set of binary expressions
    Or(ExprOr),

    /// Project an expression
    Project(ExprProject),

    /// Evaluates to a tuple value
    Record(ExprRecord),

    // TODO: delete this
    /// Reference a value from within the statement itself.
    Reference(ExprReference),

    /// A list of expressions of the same type
    List(ExprList),

    /// Evaluate a sub-statement
    Stmt(ExprStmt),

    /// Evaluates to a constant value reference
    Value(Value),
}

impl Expr {
    pub const TRUE: Expr = Expr::Value(Value::Bool(true));
    pub const FALSE: Expr = Expr::Value(Value::Bool(false));
    pub const DEFAULT: Expr = Expr::Default;

    pub fn null() -> Self {
        Self::Value(Value::Null)
    }

    /// Is a value that evaluates to null
    pub fn is_value_null(&self) -> bool {
        matches!(self, Self::Value(Value::Null))
    }

    /// Returns true if the expression is the `true` boolean expression
    pub fn is_true(&self) -> bool {
        matches!(self, Self::Value(Value::Bool(true)))
    }

    /// Returns `true` if the expression is the `false` boolean expression
    pub fn is_false(&self) -> bool {
        matches!(self, Self::Value(Value::Bool(false)))
    }

    /// Returns `true` if the expression is the default expression
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }

    /// Returns true if the expression is a constant value.
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(..))
    }

    pub fn is_stmt(&self) -> bool {
        matches!(self, Self::Stmt(..))
    }

    /// Returns true if the expression is a binary operation
    pub fn is_binary_op(&self) -> bool {
        matches!(self, Self::BinaryOp(..))
    }

    pub fn is_arg(&self) -> bool {
        matches!(self, Self::Arg(_))
    }

    /// Returns true if the expression is always non-nullable.
    ///
    /// This method is conservative and only returns true for expressions we can
    /// prove are non-nullable.
    pub fn is_always_non_nullable(&self) -> bool {
        match self {
            // A constant value is non-nullable if it's not null.
            Self::Value(value) => !value.is_null(),
            // Boolean logic expressions always evaluate to true or false.
            Self::And(_) | Self::Or(_) | Self::Not(_) => true,
            // ANY returns true if any item matches, always boolean.
            Self::Any(_) => true,
            // Comparisons always evaluate to true or false.
            Self::BinaryOp(_) => true,
            // IS NULL checks always evaluate to true or false.
            Self::IsNull(_) => true,
            // EXISTS checks always evaluate to true or false.
            Self::Exists(_) => true,
            // IN expressions always evaluate to true or false.
            Self::InList(_) | Self::InSubquery(_) => true,
            // For other expressions, we cannot prove non-nullability.
            _ => false,
        }
    }

    pub fn into_value(self) -> Value {
        match self {
            Self::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn into_stmt(self) -> ExprStmt {
        match self {
            Self::Stmt(stmt) => stmt,
            _ => todo!(),
        }
    }

    /// Returns `true` if the expression is stable
    ///
    /// An expression is stable if it yields the same value each time it is evaluated
    pub fn is_stable(&self) -> bool {
        match self {
            // Always stable - constant values
            Self::Value(_) => true,

            // Never stable - generates new values each evaluation
            Self::Default => false,

            // Stable if all children are stable
            Self::Record(expr_record) => expr_record.iter().all(|expr| expr.is_stable()),
            Self::List(expr_list) => expr_list.items.iter().all(|expr| expr.is_stable()),
            Self::Cast(expr_cast) => expr_cast.expr.is_stable(),
            Self::BinaryOp(expr_binary) => {
                expr_binary.lhs.is_stable() && expr_binary.rhs.is_stable()
            }
            Self::And(expr_and) => expr_and.iter().all(|expr| expr.is_stable()),
            Self::Any(expr_any) => expr_any.expr.is_stable(),
            Self::Or(expr_or) => expr_or.iter().all(|expr| expr.is_stable()),
            Self::IsNull(expr_is_null) => expr_is_null.expr.is_stable(),
            Self::Not(expr_not) => expr_not.expr.is_stable(),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_stable() && expr_in_list.list.is_stable()
            }
            Self::Project(expr_project) => expr_project.base.is_stable(),
            Self::Map(expr_map) => expr_map.base.is_stable() && expr_map.map.is_stable(),

            // References and statements - stable (they reference existing data)
            Self::Reference(_) | Self::Arg(_) => true,

            // Subqueries and functions - could be unstable
            // For now, conservatively mark as unstable
            Self::Stmt(_) | Self::Func(_) | Self::InSubquery(_) | Self::Exists(_) => false,
        }
    }

    /// Returns `true` if the expression is a constant expression.
    ///
    /// A constant expression is one that does not reference any external data.
    /// This means it contains no `Reference`, `Stmt`, or `Arg` expressions that
    /// reference external inputs.
    ///
    /// `Arg` expressions inside `Map` bodies *with `nesting` less than the current
    /// map depth* are local bindings (bound to the mapped element), not external
    /// inputs, and are therefore considered const in that context.
    pub fn is_const(&self) -> bool {
        self.is_const_at_depth(0)
    }

    /// Inner implementation of [`is_const`] that tracks the number of enclosing
    /// `Map` scopes. An `Arg` with `nesting < map_depth` is a local binding
    /// introduced by one of those `Map`s and does not count as external input.
    fn is_const_at_depth(&self, map_depth: usize) -> bool {
        match self {
            // Always constant
            Self::Value(_) => true,

            // Arg: local if nesting is within map_depth, otherwise external
            Self::Arg(arg) => arg.nesting < map_depth,

            // Never constant - references external data
            Self::Reference(_)
            | Self::Stmt(_)
            | Self::InSubquery(_)
            | Self::Exists(_)
            | Self::Default
            | Self::Func(_) => false,

            // Const if all children are const at the same depth
            Self::Record(expr_record) => expr_record
                .iter()
                .all(|expr| expr.is_const_at_depth(map_depth)),
            Self::List(expr_list) => expr_list
                .items
                .iter()
                .all(|expr| expr.is_const_at_depth(map_depth)),
            Self::Cast(expr_cast) => expr_cast.expr.is_const_at_depth(map_depth),
            Self::BinaryOp(expr_binary) => {
                expr_binary.lhs.is_const_at_depth(map_depth)
                    && expr_binary.rhs.is_const_at_depth(map_depth)
            }
            Self::And(expr_and) => expr_and
                .iter()
                .all(|expr| expr.is_const_at_depth(map_depth)),
            Self::Any(expr_any) => expr_any.expr.is_const_at_depth(map_depth),
            Self::Not(expr_not) => expr_not.expr.is_const_at_depth(map_depth),
            Self::Or(expr_or) => expr_or.iter().all(|expr| expr.is_const_at_depth(map_depth)),
            Self::IsNull(expr_is_null) => expr_is_null.expr.is_const_at_depth(map_depth),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_const_at_depth(map_depth)
                    && expr_in_list.list.is_const_at_depth(map_depth)
            }
            Self::Project(expr_project) => expr_project.base.is_const_at_depth(map_depth),

            // Map: base is checked at the current depth; the map body is checked
            // at depth+1 so that arg(nesting=0) in the body is treated as local.
            Self::Map(expr_map) => {
                expr_map.base.is_const_at_depth(map_depth)
                    && expr_map.map.is_const_at_depth(map_depth + 1)
            }
        }
    }

    /// Returns `true` if the expression can be evaluated.
    ///
    /// An expression can be evaluated if it doesn't contain references to external
    /// data sources like subqueries or references. Args are allowed since they
    /// represent function parameters that can be bound at evaluation time.
    pub fn is_eval(&self) -> bool {
        match self {
            // Always evaluable
            Self::Value(_) => true,

            // Args are OK for evaluation
            Self::Arg(_) => true,

            // Never evaluable - references external data
            Self::Default
            | Self::Reference(_)
            | Self::Stmt(_)
            | Self::InSubquery(_)
            | Self::Exists(_) => false,

            // Evaluable if all children are evaluable
            Self::Record(expr_record) => expr_record.iter().all(|expr| expr.is_eval()),
            Self::List(expr_list) => expr_list.items.iter().all(|expr| expr.is_eval()),
            Self::Cast(expr_cast) => expr_cast.expr.is_eval(),
            Self::BinaryOp(expr_binary) => expr_binary.lhs.is_eval() && expr_binary.rhs.is_eval(),
            Self::And(expr_and) => expr_and.iter().all(|expr| expr.is_eval()),
            Self::Any(expr_any) => expr_any.expr.is_eval(),
            Self::Or(expr_or) => expr_or.iter().all(|expr| expr.is_eval()),
            Self::Not(expr_not) => expr_not.expr.is_eval(),
            Self::IsNull(expr_is_null) => expr_is_null.expr.is_eval(),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_eval() && expr_in_list.list.is_eval()
            }
            Self::Project(expr_project) => expr_project.base.is_eval(),
            Self::Map(expr_map) => expr_map.base.is_eval() && expr_map.map.is_eval(),
            Self::Func(_) => false,
        }
    }

    pub fn map_projections(&self, f: impl FnMut(&Projection) -> Projection) -> Self {
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
    pub fn entry(&self, path: impl EntryPath) -> Option<Entry<'_>> {
        let mut ret = Entry::Expr(self);

        for step in path.step_iter() {
            ret = match ret {
                Entry::Expr(Self::Record(expr)) => Entry::Expr(&expr[step]),
                Entry::Expr(Self::List(expr)) => Entry::Expr(&expr.items[step]),
                Entry::Value(Value::Record(record))
                | Entry::Expr(Self::Value(Value::Record(record))) => Entry::Value(&record[step]),
                Entry::Value(Value::List(items)) | Entry::Expr(Self::Value(Value::List(items))) => {
                    Entry::Value(&items[step])
                }
                _ => return None,
            }
        }

        Some(ret)
    }

    #[track_caller]
    pub fn entry_mut(&mut self, path: impl EntryPath) -> EntryMut<'_> {
        let mut ret = EntryMut::Expr(self);

        for step in path.step_iter() {
            ret = match ret {
                EntryMut::Expr(Self::Record(expr)) => EntryMut::Expr(&mut expr[step]),
                EntryMut::Value(Value::Record(record))
                | EntryMut::Expr(Self::Value(Value::Record(record))) => {
                    EntryMut::Value(&mut record[step])
                }
                _ => todo!("ret={ret:#?}; step={step:#?}"),
            }
        }

        ret
    }

    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Value(Value::Null))
    }

    pub fn substitute(&mut self, input: impl Input) {
        Substitute::new(input).visit_expr_mut(self);
    }
}

impl Node for Expr {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_expr(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_expr_mut(self);
    }
}

// === Conversions ===

impl From<bool> for Expr {
    fn from(value: bool) -> Self {
        Self::Value(Value::from(value))
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Self {
        Self::Value(value.into())
    }
}

impl From<&i64> for Expr {
    fn from(value: &i64) -> Self {
        Self::Value(value.into())
    }
}

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Self::Value(value.into())
    }
}

impl From<&String> for Expr {
    fn from(value: &String) -> Self {
        Self::Value(value.into())
    }
}

impl From<&str> for Expr {
    fn from(value: &str) -> Self {
        Self::Value(value.into())
    }
}

impl From<Value> for Expr {
    fn from(value: Value) -> Self {
        Self::Value(value)
    }
}

impl<E1, E2> From<(E1, E2)> for Expr
where
    E1: Into<Self>,
    E2: Into<Self>,
{
    fn from(value: (E1, E2)) -> Self {
        Self::Record(value.into())
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::And(e) => e.fmt(f),
            Self::Any(e) => e.fmt(f),
            Self::Arg(e) => e.fmt(f),
            Self::BinaryOp(e) => e.fmt(f),
            Self::Cast(e) => e.fmt(f),
            Self::Default => write!(f, "Default"),
            Self::Exists(e) => e.fmt(f),
            Self::Func(e) => e.fmt(f),
            Self::InList(e) => e.fmt(f),
            Self::InSubquery(e) => e.fmt(f),
            Self::IsNull(e) => e.fmt(f),
            Self::Map(e) => e.fmt(f),
            Self::Not(e) => e.fmt(f),
            Self::Or(e) => e.fmt(f),
            Self::Project(e) => e.fmt(f),
            Self::Record(e) => e.fmt(f),
            Self::Reference(e) => e.fmt(f),
            Self::List(e) => e.fmt(f),
            Self::Stmt(e) => e.fmt(f),
            Self::Value(e) => e.fmt(f),
        }
    }
}
