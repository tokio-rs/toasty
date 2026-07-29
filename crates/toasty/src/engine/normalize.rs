mod paginate;
mod upsert;

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
}

impl Engine {
    pub(super) fn normalize_stmt(&self, stmt: &mut stmt::Statement) -> Result<()> {
        let mut normalize = Normalize {
            schema: &self.schema,
            capability: self.capability,
            error: None,
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
}

impl VisitMut for Normalize<'_> {
    fn visit_stmt_insert_mut(&mut self, insert: &mut stmt::Insert) {
        self.normalize_upsert_defaults(insert);
        if self.error.is_none() {
            stmt::visit_mut::visit_stmt_insert_mut(self, insert);
        }
    }

    fn visit_stmt_query_mut(&mut self, query: &mut stmt::Query) {
        self.normalize_cursor_order(query);
        if self.error.is_none() {
            stmt::visit_mut::visit_stmt_query_mut(self, query);
        }
    }
}
