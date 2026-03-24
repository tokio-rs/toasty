use std::fmt;

/// A SQL identifier (table name, column name, etc.).
///
/// Wraps a string-like value. When serialized, the identifier is quoted to
/// avoid conflicts with SQL reserved words.
///
/// # Example
///
/// ```
/// use toasty_sql::stmt::Ident;
///
/// let id = Ident::from("users");
/// assert_eq!(id.to_string(), "users");
/// ```
#[derive(Debug, Clone)]
pub struct Ident<T = String>(pub T);

impl From<&str> for Ident {
    fn from(value: &str) -> Self {
        Ident(value.into())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
