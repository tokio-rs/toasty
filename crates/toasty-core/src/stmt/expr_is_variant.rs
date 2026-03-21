use crate::schema::app::VariantId;

use super::Expr;

/// Tests whether an expression evaluates to a specific enum variant.
///
/// This is an application-level check that abstracts over the storage format
/// (unit enums stored as bare I64 vs data-carrying enums stored as Records).
/// The lowerer translates this into the appropriate DB-level comparison.
///
/// # Examples
///
/// ```text
/// is_variant(expr, VariantId(2))  // true if expr is variant 2
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsVariant {
    /// Expression evaluating to an enum value.
    pub expr: Box<Expr>,
    /// Identifies the variant to check against.
    pub variant: VariantId,
}

impl Expr {
    /// Creates a variant check expression testing whether `expr` is the given
    /// `variant`.
    pub fn is_variant(expr: impl Into<Self>, variant: VariantId) -> Self {
        ExprIsVariant {
            expr: Box::new(expr.into()),
            variant,
        }
        .into()
    }
}

impl From<ExprIsVariant> for Expr {
    fn from(value: ExprIsVariant) -> Self {
        Self::IsVariant(value)
    }
}
