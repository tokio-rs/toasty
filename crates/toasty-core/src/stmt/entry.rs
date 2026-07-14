use crate::Result;

use super::{Expr, Input, Value};

/// A borrowed reference to either an [`Expr`] or a [`Value`] within a
/// composite structure.
///
/// `Entry` is returned by navigation methods (e.g., [`Value::entry`],
/// [`Expr::entry`]) and provides a uniform way to inspect or evaluate the
/// referenced data without cloning.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::{Entry, Value};
///
/// let value = Value::from(42_i64);
/// let entry = Entry::from(&value);
/// assert!(entry.is_value());
/// assert!(!entry.is_expr());
/// ```
#[derive(Debug)]
pub enum Entry<'a> {
    /// A reference to an expression.
    Expr(&'a Expr),
    /// A reference to a value.
    Value(&'a Value),
}

impl Entry<'_> {
    /// Evaluates the entry to a value using the provided input.
    ///
    /// For `Entry::Expr`, evaluates the expression with the given input context.
    /// For `Entry::Value`, returns a clone of the value directly.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::{Entry, Value, ConstInput};
    /// let value = Value::from(42);
    /// let entry = Entry::from(&value);
    ///
    /// let result = entry.eval(ConstInput::new()).unwrap();
    /// assert_eq!(result, Value::from(42));
    /// ```
    pub fn eval(&self, input: impl Input) -> Result<Value> {
        match self {
            Entry::Expr(expr) => expr.eval(input),
            Entry::Value(value) => Ok((*value).clone()),
        }
    }

    /// Evaluates the entry as a constant expression.
    ///
    /// For `Entry::Expr`, attempts to evaluate the expression without any input context.
    /// This only succeeds if the expression is constant (contains no references or arguments).
    /// For `Entry::Value`, returns a clone of the value directly.
    ///
    /// # Errors
    ///
    /// Returns an error if the entry contains an expression that cannot be evaluated
    /// as a constant (e.g., references to columns or arguments).
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::{Entry, Value};
    /// let value = Value::from("hello");
    /// let entry = Entry::from(&value);
    ///
    /// let result = entry.eval_const().unwrap();
    /// assert_eq!(result, Value::from("hello"));
    /// ```
    pub fn eval_const(&self) -> Result<Value> {
        match self {
            Entry::Expr(expr) => expr.eval_const(),
            Entry::Value(value) => Ok((*value).clone()),
        }
    }

    /// Returns `true` if the entry is a constant expression.
    ///
    /// An entry is considered constant if it does not reference any external data:
    /// - `Entry::Value` is always constant
    /// - `Entry::Expr` is constant if the expression itself is constant
    ///   (see [`Expr::is_const`] for details)
    ///
    /// Constant entries can be evaluated without any input context.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::{Entry, Value, Expr};
    /// // Values are always constant
    /// let value = Value::from(42);
    /// let entry = Entry::from(&value);
    /// assert!(entry.is_const());
    ///
    /// // Constant expressions
    /// let expr = Expr::from(Value::from("hello"));
    /// let entry = Entry::from(&expr);
    /// assert!(entry.is_const());
    /// ```
    pub fn is_const(&self) -> bool {
        match self {
            Entry::Value(_) => true,
            Entry::Expr(expr) => expr.is_const(),
        }
    }

    /// Returns `true` if this entry contains an expression.
    pub fn is_expr(&self) -> bool {
        matches!(self, Entry::Expr(_))
    }

    /// Converts this entry to an owned [`Expr`] by cloning the contained
    /// expression or wrapping the value.
    pub fn to_expr(&self) -> Expr {
        match *self {
            Entry::Expr(expr) => expr.clone(),
            Entry::Value(value) => value.clone().into(),
        }
    }

    /// Returns `true` if this entry is `Expr::Default`.
    pub fn is_expr_default(&self) -> bool {
        matches!(self, Entry::Expr(Expr::Default))
    }

    /// Returns `true` if this entry holds a concrete value (either
    /// `Entry::Value` or `Entry::Expr(Expr::Value(_))`).
    pub fn is_value(&self) -> bool {
        matches!(self, Entry::Value(_) | Entry::Expr(Expr::Value(_)))
    }

    /// Returns `true` if this entry holds a null value.
    pub fn is_value_null(&self) -> bool {
        matches!(
            self,
            Entry::Value(Value::Null) | Entry::Expr(Expr::Value(Value::Null))
        )
    }

    /// Returns `true` if this entry holds a record, either as an
    /// `Expr::Record`, an `Expr::Value(Value::Record)`, or a bare
    /// `Value::Record`.
    pub fn is_record(&self) -> bool {
        match self {
            Entry::Expr(Expr::Record(_)) => true,
            Entry::Expr(Expr::Value(value)) => value.is_record(),
            Entry::Value(value) => value.is_record(),
            Entry::Expr(_) => false,
        }
    }

    /// Returns a reference to the contained value, or `None` if this entry
    /// holds a non-value expression.
    pub fn as_value(&self) -> Option<&Value> {
        match *self {
            Entry::Expr(Expr::Value(value)) | Entry::Value(value) => Some(value),
            _ => None,
        }
    }

    /// Returns a reference to the contained value, panicking if this
    /// entry does not hold a value.
    ///
    /// # Panics
    ///
    /// Panics if the entry is not a value.
    #[track_caller]
    pub fn as_value_unwrap(&self) -> &Value {
        self.as_value()
            .unwrap_or_else(|| panic!("expected Entry with value; actual={self:#?}"))
    }

    /// Extracts an owned [`Value`] from this entry, evaluating constant
    /// expressions if needed.
    ///
    /// # Panics
    ///
    /// Panics if the entry contains a non-constant expression.
    pub fn to_value(&self) -> Value {
        match *self {
            Entry::Expr(Expr::Value(value)) | Entry::Value(value) => value.clone(),
            Entry::Expr(expr) => expr.eval_const().unwrap_or_else(|err| {
                panic!("not const expression; entry={self:#?}; error={err:#?}")
            }),
        }
    }
}

impl<'a> From<&'a Expr> for Entry<'a> {
    fn from(value: &'a Expr) -> Self {
        Entry::Expr(value)
    }
}

impl<'a> From<&'a Value> for Entry<'a> {
    fn from(value: &'a Value) -> Self {
        Entry::Value(value)
    }
}

impl<'a> From<Entry<'a>> for Expr {
    fn from(value: Entry<'a>) -> Self {
        match value {
            Entry::Expr(expr) => expr.clone(),
            Entry::Value(value) => value.clone().into(),
        }
    }
}
