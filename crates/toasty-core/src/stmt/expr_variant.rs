use crate::schema::app::VariantId;

use super::Expr;

/// Selects a variant of an embedded enum value.
///
/// Evaluates to the payload of `base` when it holds `variant`: the record of
/// the variant's fields, in declaration order, without the discriminant.
/// Projections applied to it index that record with the variant's local
/// field positions, so `project(variant(owner, Human), [1])` is the second
/// field of `Owner::Human`, whatever the enum's storage layout.
///
/// The selection carries no check of its own. A predicate over a selection
/// must also require the variant; statement normalization in the engine
/// conjoins an [`is_variant`](Expr::is_variant) check for every selection a
/// predicate's operands reach. Lowering resolves the selection to the
/// variant's column expressions and the guard to a discriminant comparison;
/// the node never reaches a driver.
///
/// # Examples
///
/// ```text
/// variant(owner, Owner::Human)               // the Human payload
/// project(variant(owner, Owner::Human), [0]) // its first field
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprVariant {
    /// Expression evaluating to an enum value.
    pub base: Box<Expr>,

    /// The variant whose payload is selected.
    pub variant: VariantId,
}

impl Expr {
    /// Creates a variant selection: the payload of `base` as `variant`.
    pub fn variant(base: impl Into<Self>, variant: VariantId) -> Self {
        ExprVariant {
            base: Box::new(base.into()),
            variant,
        }
        .into()
    }

    /// Returns a reference to the inner [`ExprVariant`] if this is a variant
    /// selection, or `None` otherwise.
    pub fn as_variant(&self) -> Option<&ExprVariant> {
        match self {
            Self::Variant(expr_variant) => Some(expr_variant),
            _ => None,
        }
    }
}

impl From<ExprVariant> for Expr {
    fn from(value: ExprVariant) -> Self {
        Self::Variant(value)
    }
}
