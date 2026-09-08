//! Statement effect classification.
//!
//! [`classify`] walks a [`Statement`] AST and returns whether the
//! statement can be retried transparently on `connection_lost`.  The
//! pool consults this before attempting a retry; downstream consumers
//! (e.g. a runtime-checked read-only API surface) read the same
//! classification.
//!
//! See [`docs/dev/design/retry-safe-recovery.md`] for the full
//! contract.
//!
//! [`Statement`]: toasty_core::stmt::Statement
//! [`docs/dev/design/retry-safe-recovery.md`]: ../../../docs/dev/design/retry-safe-recovery.md

use toasty_core::stmt::{Delete, Insert, Statement, Update, Visit};

/// Whether a statement mutates database state.
///
/// A statement is [`Effect::ReadOnly`] if it is a [`Statement::Query`]
/// and contains no `Insert`, `Update`, or `Delete` anywhere in its
/// tree. Otherwise it is [`Effect::Mutating`].
///
/// CTE-with-mutation queries (e.g.
/// `WITH ins AS (INSERT ... RETURNING *) SELECT * FROM ins`) parse as
/// `Statement::Query` values whose `WITH` clauses contain an
/// `ExprSet::Insert`, `ExprSet::Update`, or `ExprSet::Delete` and are
/// correctly classified as `Mutating`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// The statement reads but does not mutate state.  Safe to retry
    /// after `connection_lost`.
    ReadOnly,

    /// The statement mutates state, either directly (top-level
    /// `Insert` / `Update` / `Delete`) or via an embedded sub-statement.
    /// Not safe to retry without further analysis.
    Mutating,
}

/// Classify a statement's effect on database state.
///
/// O(n) in the size of the statement tree; no schema access.
pub(crate) fn classify(stmt: &Statement) -> Effect {
    let mut walker = Walker { mutating: false };
    walker.visit_stmt(stmt);
    if walker.mutating {
        Effect::Mutating
    } else {
        Effect::ReadOnly
    }
}

struct Walker {
    mutating: bool,
}

impl Visit for Walker {
    fn visit_stmt_delete(&mut self, _: &Delete) {
        self.mutating = true;
    }

    fn visit_stmt_insert(&mut self, _: &Insert) {
        self.mutating = true;
    }

    fn visit_stmt_update(&mut self, _: &Update) {
        self.mutating = true;
    }
}

#[cfg(test)]
mod tests;
