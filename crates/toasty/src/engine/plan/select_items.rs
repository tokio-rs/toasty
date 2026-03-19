use indexmap::IndexMap;
use toasty_core::stmt;

/// A unified, ordered collection of items in a SQL SELECT / RETURNING list.
///
/// Each item is either a column reference (`ExprReference`) or a computed
/// expression (e.g., `COUNT(*)`). The position of an item in this collection
/// corresponds exactly to its position in the database result record, so there
/// is a single source of truth for positional indexing.
#[derive(Debug, Default, Clone)]
pub(crate) struct SelectItems {
    items: Vec<SelectItem>,
    /// Maps column references to their position in `items` for O(1) lookup.
    column_index: IndexMap<stmt::ExprReference, usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum SelectItem {
    Column(stmt::ExprReference),
    Func(stmt::Expr),
}

impl SelectItems {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Insert a column reference. Returns its position in the result record.
    /// If the column was already present, returns the existing position
    /// (deduplication, matching the old `IndexSet` behavior).
    pub(crate) fn insert_column(&mut self, expr_ref: stmt::ExprReference) -> usize {
        if let Some(&pos) = self.column_index.get(&expr_ref) {
            return pos;
        }
        let pos = self.items.len();
        self.items.push(SelectItem::Column(expr_ref));
        self.column_index.insert(expr_ref, pos);
        pos
    }

    /// Insert a column reference, returning `(position, inserted)` — mirrors
    /// `IndexSet::insert_full`.
    pub(crate) fn insert_column_full(&mut self, expr_ref: stmt::ExprReference) -> (usize, bool) {
        if let Some(&pos) = self.column_index.get(&expr_ref) {
            return (pos, false);
        }
        let pos = self.items.len();
        self.items.push(SelectItem::Column(expr_ref));
        self.column_index.insert(expr_ref, pos);
        (pos, true)
    }

    /// Append a function expression. Returns its position in the result record.
    pub(crate) fn insert_func(&mut self, expr: stmt::Expr) -> usize {
        let pos = self.items.len();
        self.items.push(SelectItem::Func(expr));
        pos
    }

    /// Look up the result-record position of a column reference.
    pub(crate) fn get_column_index(&self, expr_ref: &stmt::ExprReference) -> Option<usize> {
        self.column_index.get(expr_ref).copied()
    }

    /// Look up the result-record position of a function expression by equality.
    pub(crate) fn get_func_index(&self, expr: &stmt::Expr) -> Option<usize> {
        self.items
            .iter()
            .position(|item| matches!(item, SelectItem::Func(e) if e == expr))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when the query should return only a count of matching rows:
    /// exactly one item and it's a `COUNT(*)`.
    pub(crate) fn is_count_only(&self) -> bool {
        self.items.len() == 1
            && matches!(
                &self.items[0],
                SelectItem::Func(stmt::Expr::Func(stmt::ExprFunc::Count(_)))
            )
    }

    /// Build a `Returning` clause from the items in this collection.
    pub(crate) fn to_returning(&self) -> stmt::Returning {
        stmt::Returning::from_expr_iter(self.items.iter().map(|item| match item {
            SelectItem::Column(expr_ref) => stmt::Expr::from(*expr_ref),
            SelectItem::Func(expr) => expr.clone(),
        }))
    }

    /// Extract just the column references, preserving their relative insertion
    /// order. Used when passing columns to NoSQL MIR operations.
    pub(crate) fn columns_only(&self) -> indexmap::IndexSet<stmt::ExprReference> {
        debug_assert!(self.column_index.len() == self.items.len());
        self.column_index.keys().copied().collect()
    }

    /// Iterate over column references in insertion order.
    pub(crate) fn column_refs(&self) -> impl Iterator<Item = &stmt::ExprReference> {
        self.column_index.keys()
    }

    pub(crate) fn infer_ty(&self, cx: &stmt::ExprContext) -> stmt::Type {
        stmt::Type::Record(
            self.items
                .iter()
                .map(|item| match item {
                    SelectItem::Column(expr_reference) => {
                        cx.infer_expr_reference_ty(expr_reference)
                    }
                    SelectItem::Func(expr) => cx.infer_expr_ty(expr, &[]),
                })
                .collect(),
        )
    }
}
