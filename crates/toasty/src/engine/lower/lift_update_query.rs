//! Pre-lowering rewrite that converts `Update { target: Query(q), .. }`
//! into `Update { target: Model(m), filter: filter ∧ q.filter, .. }`.
//!
//! [`LiftUpdateQuery`] runs as a whole-statement pre-pass before the
//! main lowering walk.  The visitor overrides `visit_stmt_update_mut`
//! to lift the inner select's source model out as the new
//! `UpdateTarget::Model` and merge its filter into the outer update's
//! filter.
//!
//! `UpdateTarget::Query` is the app-shaped form of "update the rows
//! produced by this query".  The db-level form is always
//! `UpdateTarget::Table`; the lowering walk's
//! `LowerStatement::visit_update_target_mut` panics on
//! `UpdateTarget::Query`.  The lift fires before lowering so the walk
//! only ever sees `UpdateTarget::Model`, which it converts to
//! `UpdateTarget::Table`.

use toasty_core::stmt::{self, VisitMut};

/// Pre-lowering pass that lifts `UpdateTarget::Query` into
/// `UpdateTarget::Model` with the inner query's filter merged onto the
/// outer update.
pub(super) struct LiftUpdateQuery;

impl LiftUpdateQuery {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn rewrite(&mut self, stmt: &mut stmt::Statement) {
        self.visit_mut(stmt);
    }
}

impl VisitMut for LiftUpdateQuery {
    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        if let stmt::UpdateTarget::Query(query) = &mut stmt.target {
            let stmt::ExprSet::Select(select) = &mut query.body else {
                todo!()
            };

            assert!(select.returning.is_model());

            stmt.filter.add_filter(select.filter.take());
            stmt.target = stmt::UpdateTarget::Model(select.source.model_id_unwrap());
        }

        stmt::visit_mut::visit_stmt_update_mut(self, stmt);
    }
}
