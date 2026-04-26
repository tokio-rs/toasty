use crate::stmt::{ExprExists, Input};

use super::{
    Entry, EntryMut, EntryPath, ExprAnd, ExprAny, ExprArg, ExprBinaryOp, ExprCast, ExprError,
    ExprFunc, ExprInList, ExprInSubquery, ExprIsNull, ExprIsVariant, ExprLet, ExprLike, ExprList,
    ExprMap, ExprMatch, ExprNot, ExprOr, ExprProject, ExprRecord, ExprStartsWith, ExprStmt, Node,
    Projection, Substitute, Value, Visit, VisitMut, expr_reference::ExprReference,
};
use std::fmt;

/// An expression node in Toasty's query AST.
///
/// `Expr` is the central type in the statement intermediate representation. Every
/// filter, projection, value, and computed result in a Toasty query is
/// represented as an `Expr` tree. The query engine compiles these trees through
/// several phases (simplify, lower, plan, execute) before they reach a database
/// driver.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Expr, Value};
///
/// // Constant value expressions
/// let t = Expr::TRUE;
/// assert!(t.is_true());
///
/// let n = Expr::null();
/// assert!(n.is_value_null());
///
/// // From conversions
/// let i: Expr = 42i64.into();
/// assert!(i.is_value());
/// ```
#[derive(Clone, PartialEq)]
pub enum Expr {
    /// Logical AND of multiple expressions. See [`ExprAnd`].
    And(ExprAnd),

    /// Returns `true` if any item in a collection is truthy. See [`ExprAny`].
    Any(ExprAny),

    /// Positional argument placeholder. See [`ExprArg`].
    Arg(ExprArg),

    /// String prefix match: `starts_with(expr, prefix)`. See [`ExprBeginsWith`].
    StartsWith(ExprStartsWith),

    /// Binary comparison or arithmetic operation. See [`ExprBinaryOp`].
    BinaryOp(ExprBinaryOp),

    /// Type cast. See [`ExprCast`].
    Cast(ExprCast),

    /// Instructs the database to use its default value for a column. Useful for
    /// auto-increment fields and other columns with server-side defaults.
    Default,

    /// An error expression that fails evaluation with a message. See [`ExprError`].
    Error(ExprError),

    /// `[NOT] EXISTS(SELECT ...)` check. See [`ExprExists`].
    Exists(ExprExists),

    /// Aggregate or scalar function call. See [`ExprFunc`].
    Func(ExprFunc),

    /// An **unresolved** reference to a name (e.g. a column name in a DDL
    /// context).
    ///
    /// Unlike [`Expr::Reference`] / [`ExprReference`], which hold **resolved**
    /// index-based references into the schema, `Ident` carries only the raw
    /// name string. It is used in contexts where schema resolution is not
    /// applicable, such as CHECK constraints in CREATE TABLE statements.
    Ident(String),

    /// `expr IN (list)` membership test. See [`ExprInList`].
    InList(ExprInList),

    /// `expr IN (SELECT ...)` membership test. See [`ExprInSubquery`].
    InSubquery(ExprInSubquery),

    /// `IS [NOT] NULL` check. Separate from binary operators because of
    /// three-valued logic semantics in SQL. See [`ExprIsNull`].
    IsNull(ExprIsNull),

    /// Tests whether a value is a specific enum variant. See [`ExprIsVariant`].
    IsVariant(ExprIsVariant),

    /// Scoped binding expression (transient -- inlined before planning).
    /// See [`ExprLet`].
    Let(ExprLet),

    /// SQL `LIKE` pattern match: `expr LIKE pattern`. See [`ExprLike`].
    Like(ExprLike),

    /// Applies a transformation to each item in a collection. See [`ExprMap`].
    Map(ExprMap),

    /// Pattern-match dispatching on a subject. See [`ExprMatch`].
    Match(ExprMatch),

    /// Boolean negation. See [`ExprNot`].
    Not(ExprNot),

    /// Logical OR of multiple expressions. See [`ExprOr`].
    Or(ExprOr),

    /// Field projection from a composite value. See [`ExprProject`].
    Project(ExprProject),

    /// Fixed-size heterogeneous tuple of expressions. See [`ExprRecord`].
    Record(ExprRecord),

    // TODO: delete this
    /// Reference to a field, column, or model in the current or an outer query
    /// scope. See [`ExprReference`].
    Reference(ExprReference),

    /// Ordered, homogeneous collection of expressions. See [`ExprList`].
    List(ExprList),

    /// Embedded sub-statement (e.g., a subquery). See [`ExprStmt`].
    Stmt(ExprStmt),

    /// Constant value. See [`Value`].
    Value(Value),
}

impl Expr {
    /// The boolean `true` constant expression.
    pub const TRUE: Expr = Expr::Value(Value::Bool(true));

    /// The boolean `false` constant expression.
    pub const FALSE: Expr = Expr::Value(Value::Bool(false));

    /// Alias for [`Expr::Default`] as a constant.
    pub const DEFAULT: Expr = Expr::Default;

    /// Creates a null value expression.
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

    /// Returns `true` if the expression can never evaluate to `true`.
    ///
    /// In SQL's three-valued logic, both `false` and `null` are unsatisfiable:
    /// a filter producing either value will never match any rows.
    pub fn is_unsatisfiable(&self) -> bool {
        self.is_false() || self.is_value_null()
    }

    /// Returns `true` if the expression is the default expression
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default)
    }

    /// Returns true if the expression is a constant value.
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(..))
    }

    /// Returns `true` if the expression is a sub-statement.
    pub fn is_stmt(&self) -> bool {
        matches!(self, Self::Stmt(..))
    }

    /// Returns true if the expression is a binary operation
    pub fn is_binary_op(&self) -> bool {
        matches!(self, Self::BinaryOp(..))
    }

    /// Returns `true` if the expression is an argument placeholder.
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
            // Variant checks always evaluate to true or false.
            Self::IsVariant(_) => true,
            // EXISTS checks always evaluate to true or false.
            Self::Exists(_) => true,
            // IN expressions always evaluate to true or false.
            Self::InList(_) | Self::InSubquery(_) => true,
            // For other expressions, we cannot prove non-nullability.
            _ => false,
        }
    }

    /// Consumes the expression and returns the inner [`Value`].
    ///
    /// # Panics
    ///
    /// Panics (via `todo!()`) if `self` is not an `Expr::Value`.
    pub fn into_value(self) -> Value {
        match self {
            Self::Value(value) => value,
            _ => todo!(),
        }
    }

    /// Consumes the expression and returns the inner [`ExprStmt`].
    ///
    /// # Panics
    ///
    /// Panics (via `todo!()`) if `self` is not an `Expr::Stmt`.
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

            // Unresolved identifiers refer to external state (e.g. a column)
            Self::Ident(_) => false,

            // Never stable - generates new values each evaluation
            Self::Default => false,

            // Error expressions are stable (they always produce the same error)
            Self::Error(_) => true,

            // Stable if all children are stable
            Self::Record(expr_record) => expr_record.iter().all(|expr| expr.is_stable()),
            Self::List(expr_list) => expr_list.items.iter().all(|expr| expr.is_stable()),
            Self::Cast(expr_cast) => expr_cast.expr.is_stable(),
            Self::StartsWith(e) => e.expr.is_stable() && e.prefix.is_stable(),
            Self::Like(e) => e.expr.is_stable() && e.pattern.is_stable(),
            Self::BinaryOp(expr_binary) => {
                expr_binary.lhs.is_stable() && expr_binary.rhs.is_stable()
            }
            Self::And(expr_and) => expr_and.iter().all(|expr| expr.is_stable()),
            Self::Any(expr_any) => expr_any.expr.is_stable(),
            Self::Or(expr_or) => expr_or.iter().all(|expr| expr.is_stable()),
            Self::IsNull(expr_is_null) => expr_is_null.expr.is_stable(),
            Self::IsVariant(expr_is_variant) => expr_is_variant.expr.is_stable(),
            Self::Not(expr_not) => expr_not.expr.is_stable(),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_stable() && expr_in_list.list.is_stable()
            }
            Self::Project(expr_project) => expr_project.base.is_stable(),
            Self::Let(expr_let) => {
                expr_let.bindings.iter().all(|b| b.is_stable()) && expr_let.body.is_stable()
            }
            Self::Map(expr_map) => expr_map.base.is_stable() && expr_map.map.is_stable(),
            Self::Match(expr_match) => {
                expr_match.subject.is_stable()
                    && expr_match.arms.iter().all(|arm| arm.expr.is_stable())
            }

            // References and statements - stable (they reference existing data)
            Self::Reference(_) | Self::Arg(_) => true,

            // Subqueries and functions - could be unstable
            // For now, conservatively mark as unstable
            Self::Stmt(_) | Self::Func(_) | Self::InSubquery(_) | Self::Exists(_) => false,
        }
    }

    /// Returns `true` if `self` and `other` are syntactically identical **and**
    /// both sides are stable.
    ///
    /// This is the soundness-preserving comparison used by simplification
    /// rules that rewrite on the assumption that two equal sub-expressions
    /// produce the same value (idempotent, absorption, complement,
    /// range-to-equality, OR-to-IN, factoring, variant tautology).
    ///
    /// Syntactic identity alone is not enough: `LAST_INSERT_ID() =
    /// LAST_INSERT_ID()` is two independent evaluations and may yield
    /// different values, so rewriting `a AND a` to `a` would be unsound when
    /// `a` is non-deterministic. Gating on [`Self::is_stable`] excludes any
    /// sub-expression whose value may change across evaluations.
    pub fn is_equivalent_to(&self, other: &Self) -> bool {
        self == other && self.is_stable()
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

            // Unresolved identifiers reference external data
            Self::Ident(_) => false,

            // Arg: local if nesting is within map_depth, otherwise external
            Self::Arg(arg) => arg.nesting < map_depth,

            // Error expressions are constant (no external data)
            Self::Error(_) => true,

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
            Self::StartsWith(e) => {
                e.expr.is_const_at_depth(map_depth) && e.prefix.is_const_at_depth(map_depth)
            }
            Self::Like(e) => {
                e.expr.is_const_at_depth(map_depth) && e.pattern.is_const_at_depth(map_depth)
            }
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
            Self::IsVariant(expr_is_variant) => expr_is_variant.expr.is_const_at_depth(map_depth),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_const_at_depth(map_depth)
                    && expr_in_list.list.is_const_at_depth(map_depth)
            }
            Self::Project(expr_project) => expr_project.base.is_const_at_depth(map_depth),

            // Let: binding is checked at the current depth; the body is checked
            // at depth+1 so that arg(nesting=0) in the body is treated as local.
            Self::Let(expr_let) => {
                expr_let
                    .bindings
                    .iter()
                    .all(|b| b.is_const_at_depth(map_depth))
                    && expr_let.body.is_const_at_depth(map_depth + 1)
            }
            // Map: base is checked at the current depth; the map body is checked
            // at depth+1 so that arg(nesting=0) in the body is treated as local.
            Self::Map(expr_map) => {
                expr_map.base.is_const_at_depth(map_depth)
                    && expr_map.map.is_const_at_depth(map_depth + 1)
            }
            Self::Match(expr_match) => {
                expr_match.subject.is_const_at_depth(map_depth)
                    && expr_match
                        .arms
                        .iter()
                        .all(|arm| arm.expr.is_const_at_depth(map_depth))
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

            // Unresolved identifiers cannot be evaluated
            Self::Ident(_) => false,

            // Args are OK for evaluation
            Self::Arg(_) => true,

            // Error expressions are evaluable (they produce an error)
            Self::Error(_) => true,

            // Never evaluable - references external data or requires a database driver
            Self::Default
            | Self::Reference(_)
            | Self::Stmt(_)
            | Self::InSubquery(_)
            | Self::Exists(_)
            | Self::StartsWith(_)
            | Self::Like(_) => false,

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
            Self::IsVariant(expr_is_variant) => expr_is_variant.expr.is_eval(),
            Self::InList(expr_in_list) => {
                expr_in_list.expr.is_eval() && expr_in_list.list.is_eval()
            }
            Self::Project(expr_project) => expr_project.base.is_eval(),
            Self::Let(expr_let) => {
                expr_let.bindings.iter().all(|b| b.is_eval()) && expr_let.body.is_eval()
            }
            Self::Map(expr_map) => expr_map.base.is_eval() && expr_map.map.is_eval(),
            Self::Match(expr_match) => {
                expr_match.subject.is_eval() && expr_match.arms.iter().all(|arm| arm.expr.is_eval())
            }
            Self::Func(_) => false,
        }
    }

    /// Returns a clone of this expression with all [`Projection`] nodes
    /// transformed by `f`.
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

    /// Navigates into a nested record or list expression by `path` and returns
    /// a read-only [`Entry`] reference.
    ///
    /// Returns `None` if the path cannot be followed (e.g., the expression is
    /// not a record or list at the expected depth).
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

    /// Navigates into a nested record or list expression by `path` and returns
    /// a mutable [`EntryMut`] reference.
    ///
    /// # Panics
    ///
    /// Panics if the path cannot be followed on the current expression shape.
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

    /// Takes the expression out, leaving `Expr::Value(Value::Null)` in its
    /// place. Equivalent to `std::mem::replace(self, Expr::null())`.
    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Value(Value::Null))
    }

    /// Replaces every [`ExprArg`] in this expression tree with the
    /// corresponding value from `input`.
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
            Self::StartsWith(e) => e.fmt(f),
            Self::BinaryOp(e) => e.fmt(f),
            Self::Cast(e) => e.fmt(f),
            Self::Default => write!(f, "Default"),
            Self::Error(e) => e.fmt(f),
            Self::Exists(e) => e.fmt(f),
            Self::Func(e) => e.fmt(f),
            Self::Ident(e) => write!(f, "Ident({e:?})"),
            Self::InList(e) => e.fmt(f),
            Self::InSubquery(e) => e.fmt(f),
            Self::IsNull(e) => e.fmt(f),
            Self::IsVariant(e) => e.fmt(f),
            Self::Let(e) => e.fmt(f),
            Self::Like(e) => e.fmt(f),
            Self::Map(e) => e.fmt(f),
            Self::Match(e) => e.fmt(f),
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
