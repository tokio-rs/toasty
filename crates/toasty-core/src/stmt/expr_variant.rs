use crate::schema::app::VariantId;

use super::{Expr, ExprAnd, ExprOr, Query, Visit};

/// Selects a variant of an embedded enum value.
///
/// Evaluates to the payload of `base` when it holds `variant`: the record of
/// the variant's fields, in declaration order, without the discriminant.
/// Projections applied to it index that record with the variant's local
/// field positions, so `project(variant(owner, Human), [1])` is the second
/// field of `Owner::Human`, whatever the enum's storage layout.
///
/// The selection carries no check of its own. A predicate over a selection
/// must also require the variant — see [`Expr::with_variant_guards`], which
/// the typed layer applies when it builds a predicate. Lowering resolves the
/// selection to the variant's column expressions and the guard to a
/// discriminant comparison; the node never reaches a driver.
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

    /// Returns the variant checks a predicate over this expression requires.
    ///
    /// Every [`ExprVariant`] reached from `self` contributes an
    /// [`is_variant`](Expr::is_variant) check on its base, outer selections
    /// before the selections nested inside them, without duplicates. The walk
    /// stops at `And` and `Or` operands and at subqueries: those are
    /// predicates in their own right and already carry their guards.
    pub fn variant_guards(&self) -> Vec<Self> {
        struct Collect(Vec<Expr>);

        impl Visit for Collect {
            fn visit_expr_variant(&mut self, i: &ExprVariant) {
                // The base holds the enclosing selections, so its guards come
                // first.
                self.visit_expr(&i.base);

                let guard = Expr::is_variant((*i.base).clone(), i.variant);
                if !self.0.contains(&guard) {
                    self.0.push(guard);
                }
            }

            fn visit_expr_and(&mut self, _: &ExprAnd) {}

            fn visit_expr_or(&mut self, _: &ExprOr) {}

            fn visit_stmt_query(&mut self, _: &Query) {}
        }

        let mut collect = Collect(vec![]);
        collect.visit_expr(self);
        collect.0
    }

    /// Conjoins the variant checks a predicate requires with the predicate.
    ///
    /// `self` is a predicate whose operands may select enum variants. The
    /// result is `is_variant(..) AND .. AND self`, one check per selection
    /// found by [`variant_guards`](Expr::variant_guards), or `self` unchanged
    /// when there is none. Guards are fixed here, before the predicate is
    /// combined with others: negating the result negates the guarded
    /// predicate as a whole.
    pub fn with_variant_guards(self) -> Self {
        let mut guards = self.variant_guards();

        if guards.is_empty() {
            return self;
        }

        guards.push(self);
        Self::and_from_vec(guards)
    }
}

impl From<ExprVariant> for Expr {
    fn from(value: ExprVariant) -> Self {
        Self::Variant(value)
    }
}
