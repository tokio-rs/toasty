use toasty_core::stmt;

/// A wrapper that carries type information alongside statements and expressions.
/// This preserves semantic type information throughout the query pipeline.
#[derive(Debug, Clone)]
pub(crate) struct Typed<T> {
    /// The statement/expression
    pub(crate) value: T,

    /// The type this evaluates to
    pub(crate) ty: stmt::Type,
}

impl<T> Typed<T> {
    /// Create a new typed wrapper
    pub(crate) fn new(value: T, ty: stmt::Type) -> Self {
        Self { value, ty }
    }
}
