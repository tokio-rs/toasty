mod paginate;
mod upsert;
mod variant_guards;

#[cfg(test)]
mod tests;

use toasty_core::{
    Error, Result, Schema,
    driver::Capability,
    stmt::{self, VisitMut},
};

use super::Engine;

/// Normalizes application-level statements in one recursive traversal.
///
/// Node-specific helpers may inspect or mutate the node they receive, but they
/// must not recurse. The [`VisitMut`] implementation owns traversal so adding a
/// normalization rule does not add another full AST walk.
struct Normalize<'a> {
    schema: &'a Schema,
    capability: &'a Capability,
    error: Option<Error>,

    /// Variant guards the enclosing conjunctions state, innermost last.
    ///
    /// A predicate inside `is_variant(x, v) AND ..` need not repeat that
    /// guard. The stack is scoped to the statement being walked: a subquery
    /// starts empty.
    guards: Vec<stmt::Expr>,
}

impl Engine {
    pub(super) fn normalize_stmt(&self, stmt: &mut stmt::Statement) -> Result<()> {
        let mut normalize = Normalize {
            schema: &self.schema,
            capability: &self.capability,
            error: None,
            guards: vec![],
        };
        normalize.visit_stmt_mut(stmt);

        match normalize.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Normalize<'_> {
    fn record(&mut self, error: Error) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }

    /// Walks a nested statement with its own guard scope.
    fn scoped(&mut self, walk: impl FnOnce(&mut Self)) {
        let guards = std::mem::take(&mut self.guards);
        walk(self);
        self.guards = guards;
    }
}

impl VisitMut for Normalize<'_> {
    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        if let stmt::Expr::And(and) = expr {
            self.normalize_conjunction(and);
        } else {
            stmt::visit_mut::visit_expr_mut(self, expr);
            self.normalize_predicate_guards(expr);
        }
    }

    fn visit_stmt_mut(&mut self, stmt: &mut stmt::Statement) {
        self.scoped(|this| stmt::visit_mut::visit_stmt_mut(this, stmt));
    }

    fn visit_stmt_insert_mut(&mut self, insert: &mut stmt::Insert) {
        self.normalize_upsert_defaults(insert);
        if self.error.is_none() {
            stmt::visit_mut::visit_stmt_insert_mut(self, insert);
        }
    }

    fn visit_stmt_query_mut(&mut self, query: &mut stmt::Query) {
        self.normalize_cursor_order(query);
        if self.error.is_none() {
            self.scoped(|this| stmt::visit_mut::visit_stmt_query_mut(this, query));
        }
    }
}
