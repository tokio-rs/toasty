use std::ops::{Deref, DerefMut};

use toasty_core::stmt;

/// Represents an item that the engine can select from the database.
///
/// This generalizes `ExprReference` to support both column references and
/// computed expressions like `COUNT(*)`. Using an enum that derives `Hash` and
/// `Eq` allows the planner to continue deduplicating select items via
/// `SelectItems`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub(crate) enum SelectItem {
    /// A reference to a column or field — the traditional case.
    ExprReference(stmt::ExprReference),

    /// The `COUNT(*)` aggregate. SQL-only.
    CountStar,
}

impl SelectItem {
    /// Returns the inner `ExprReference`, panicking if this is not one.
    #[track_caller]
    pub(crate) fn as_expr_reference_unwrap(&self) -> &stmt::ExprReference {
        match self {
            SelectItem::ExprReference(r) => r,
            other => panic!("expected ExprReference, got {other:?}"),
        }
    }

    /// Infer the type of this select item given the expression context.
    pub(crate) fn infer_ty(&self, cx: &stmt::ExprContext<'_>) -> stmt::Type {
        match self {
            SelectItem::ExprReference(expr_reference) => cx.infer_expr_reference_ty(expr_reference),
            SelectItem::CountStar => stmt::Type::U64,
        }
    }

    /// Convert this select item into the corresponding expression.
    pub(crate) fn to_expr(&self) -> stmt::Expr {
        match self {
            SelectItem::ExprReference(expr_reference) => stmt::Expr::from(*expr_reference),
            SelectItem::CountStar => stmt::Expr::count_star(),
        }
    }
}

impl From<stmt::ExprReference> for SelectItem {
    fn from(value: stmt::ExprReference) -> Self {
        SelectItem::ExprReference(value)
    }
}

/// A set of [`SelectItem`]s backed by an [`IndexSet`](indexmap::IndexSet) for
/// deduplication and index-based lookup.
#[derive(Debug, Default, Clone)]
pub(crate) struct SelectItems(indexmap::IndexSet<SelectItem>);

impl SelectItems {
    pub(crate) fn new() -> Self {
        Self(indexmap::IndexSet::new())
    }

    /// Find the index of an `ExprReference` item, returning `None` if not
    /// present.
    pub(crate) fn try_get_index_of_expr_reference(
        &self,
        expr_reference: impl Into<stmt::ExprReference>,
    ) -> Option<usize> {
        let item = SelectItem::ExprReference(expr_reference.into());
        self.0.get_index_of(&item)
    }

    /// Find the index of an `ExprReference` item.
    pub(crate) fn get_index_of_expr_reference(
        &self,
        expr_reference: impl Into<stmt::ExprReference>,
    ) -> usize {
        self.try_get_index_of_expr_reference(expr_reference)
            .unwrap()
    }

    /// Find the index of the `CountStar` item.
    pub(crate) fn get_index_of_count_star(&self) -> usize {
        self.0.get_index_of(&SelectItem::CountStar).unwrap()
    }

    /// Returns `Type::List(Type::Record(field_tys))` where each `field_ty` is
    /// inferred from the corresponding select item.
    pub(crate) fn infer_record_list_ty(&self, cx: &stmt::ExprContext<'_>) -> stmt::Type {
        let field_tys = self.0.iter().map(|item| item.infer_ty(cx)).collect();
        stmt::Type::list(stmt::Type::Record(field_tys))
    }

    /// Extract only the `ExprReference` items from this set.
    ///
    /// Used by the NoSQL path where only column references are valid.
    pub(crate) fn extract_expr_references(&self) -> indexmap::IndexSet<stmt::ExprReference> {
        self.0
            .iter()
            .map(|item| *item.as_expr_reference_unwrap())
            .collect()
    }
}

impl<'a> IntoIterator for &'a SelectItems {
    type Item = &'a SelectItem;
    type IntoIter = indexmap::set::Iter<'a, SelectItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Deref for SelectItems {
    type Target = indexmap::IndexSet<SelectItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SelectItems {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
