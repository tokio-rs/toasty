use toasty_core::stmt;

/// Represents an item that the engine can select from the database.
///
/// This generalizes `ExprReference` to support both column references and
/// computed expressions like `COUNT(*)`. Using an enum that derives `Hash` and
/// `Eq` allows the planner to continue deduplicating select items via
/// `IndexSet<SelectItem>`.
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

    /// Find the index of an `ExprReference` item in a selection set.
    pub(crate) fn get_index_of_expr_reference(
        selection: &indexmap::IndexSet<SelectItem>,
        expr_reference: impl Into<stmt::ExprReference>,
    ) -> usize {
        let item = SelectItem::ExprReference(expr_reference.into());
        selection.get_index_of(&item).unwrap()
    }

    /// Find the index of the `CountStar` item in a selection set.
    pub(crate) fn get_index_of_count_star(selection: &indexmap::IndexSet<SelectItem>) -> usize {
        selection.get_index_of(&SelectItem::CountStar).unwrap()
    }
}

impl SelectItem {
    /// Extract only the `ExprReference` items from a set of `SelectItem`s.
    ///
    /// Used by the NoSQL path where only column references are valid.
    pub(crate) fn extract_expr_references(
        items: &indexmap::IndexSet<SelectItem>,
    ) -> indexmap::IndexSet<stmt::ExprReference> {
        items
            .iter()
            .map(|item| *item.as_expr_reference_unwrap())
            .collect()
    }
}

impl From<stmt::ExprReference> for SelectItem {
    fn from(value: stmt::ExprReference) -> Self {
        SelectItem::ExprReference(value)
    }
}
