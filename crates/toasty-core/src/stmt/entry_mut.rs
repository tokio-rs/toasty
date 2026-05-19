use super::{Expr, Value};

/// A mutable reference to either an [`Expr`] or a [`Value`] within a
/// composite structure.
///
/// This is the mutable counterpart to [`Entry`](super::Entry), used for
/// in-place modification of nested expressions or values.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{EntryMut, Expr, Value};
///
/// let mut expr = Expr::from(Value::from(42_i64));
/// let mut entry = EntryMut::from(&mut expr);
/// assert!(entry.is_expr());
/// ```
#[derive(Debug)]
pub enum EntryMut<'a> {
    /// A mutable reference to an expression.
    Expr(&'a mut Expr),
    /// A mutable reference to a value.
    Value(&'a mut Value),
}

impl EntryMut<'_> {
    /// Returns a reference to the contained expression, or `None`.
    pub fn as_expr(&self) -> Option<&Expr> {
        match self {
            EntryMut::Expr(e) => Some(e),
            _ => None,
        }
    }

    /// Returns a reference to the contained expression, panicking if not an expression.
    ///
    /// # Panics
    ///
    /// Panics if this entry is not `EntryMut::Expr`.
    #[track_caller]
    pub fn as_expr_unwrap(&self) -> &Expr {
        self.as_expr()
            .unwrap_or_else(|| panic!("expected EntryMut::Expr; actual={self:#?}"))
    }

    /// Returns a mutable reference to the contained expression, or `None`.
    pub fn as_expr_mut(&mut self) -> Option<&mut Expr> {
        match self {
            EntryMut::Expr(e) => Some(e),
            _ => None,
        }
    }

    /// Returns a mutable reference to the contained expression, panicking if not an expression.
    ///
    /// # Panics
    ///
    /// Panics if this entry is not `EntryMut::Expr`.
    #[track_caller]
    pub fn as_expr_mut_unwrap(&mut self) -> &mut Expr {
        match self {
            EntryMut::Expr(e) => e,
            _ => panic!("expected EntryMut::Expr"),
        }
    }

    /// Returns `true` if this entry holds an expression.
    pub fn is_expr(&self) -> bool {
        matches!(self, EntryMut::Expr(_))
    }

    /// Returns `true` if this entry holds a statement expression.
    pub fn is_statement(&self) -> bool {
        matches!(self, EntryMut::Expr(e) if e.is_stmt())
    }

    /// Returns `true` if this entry holds a concrete value.
    pub fn is_value(&self) -> bool {
        matches!(self, EntryMut::Value(_) | EntryMut::Expr(Expr::Value(_)))
    }

    /// Returns `true` if this entry holds a null value.
    pub fn is_value_null(&self) -> bool {
        matches!(
            self,
            EntryMut::Value(Value::Null) | EntryMut::Expr(Expr::Value(Value::Null))
        )
    }

    /// Returns `true` if this entry holds a record, either as an
    /// `Expr::Record`, an `Expr::Value(Value::Record)`, or a bare
    /// `Value::Record`.
    pub fn is_record(&self) -> bool {
        match self {
            EntryMut::Expr(Expr::Record(_)) => true,
            EntryMut::Expr(Expr::Value(value)) => value.is_record(),
            EntryMut::Value(value) => value.is_record(),
            EntryMut::Expr(_) => false,
        }
    }

    /// Returns `true` if this entry is `Expr::Default`.
    pub fn is_default(&self) -> bool {
        matches!(self, EntryMut::Expr(Expr::Default))
    }

    /// Takes the contained expression or value, replacing it with a default.
    pub fn take(&mut self) -> Expr {
        match self {
            EntryMut::Expr(expr) => expr.take(),
            EntryMut::Value(value) => value.take().into(),
        }
    }

    /// Replaces the contents of this entry with `expr`.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Value` entry and `expr` is not `Expr::Value`.
    pub fn insert(&mut self, expr: Expr) {
        match self {
            EntryMut::Expr(e) => **e = expr,
            EntryMut::Value(e) => match expr {
                Expr::Value(value) => **e = value,
                _ => panic!("cannot store expression in value entry"),
            },
        }
    }
}

impl<'a> From<&'a mut Expr> for EntryMut<'a> {
    fn from(value: &'a mut Expr) -> Self {
        EntryMut::Expr(value)
    }
}

impl<'a> From<&'a mut Value> for EntryMut<'a> {
    fn from(value: &'a mut Value) -> Self {
        EntryMut::Value(value)
    }
}
