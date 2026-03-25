use crate::stmt::Statement;

use super::{Expr, Projection};

use indexmap::{Equivalent, IndexMap};
use std::{hash::Hash, ops};

/// An ordered map of field assignments for an [`Update`](super::Update) statement.
///
/// Each entry maps a field projection (identifying which field to change) to an
/// [`Assignment`] (how to change it). The insertion order is preserved.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Assignments, Expr, Projection};
///
/// let mut assignments = Assignments::default();
/// assert!(assignments.is_empty());
///
/// assignments.set(Projection::single(0), Expr::null());
/// assert_eq!(assignments.len(), 1);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Assignments {
    /// Map from field projection to assignment. The projection may reference an
    /// application-level model field or a lowered table column. Supports both
    /// single-step (e.g., `[0]`) and multi-step projections (e.g., `[0, 1]`
    /// for nested fields).
    assignments: IndexMap<Projection, Assignment>,

    /// Additional assignments for projections that already have a primary entry
    /// with a different operation type. This supports combining insert and
    /// remove operations on the same has-many field in a single update.
    extra: Vec<(Projection, Assignment)>,
}

/// A single field assignment within an [`Update`](super::Update) statement.
///
/// Pairs an [`AssignmentOp`] (what kind of change) with an [`Expr`] (the
/// value to use).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Assignment, AssignmentOp, Expr};
///
/// let assignment = Assignment {
///     op: AssignmentOp::Set,
///     expr: Expr::null(),
/// };
/// assert!(assignment.op.is_set());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// The kind of assignment operation.
    pub op: AssignmentOp,

    /// The expression providing the value for this assignment.
    pub expr: Expr,
}

/// The kind of operation performed by an [`Assignment`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::AssignmentOp;
///
/// assert!(AssignmentOp::Set.is_set());
/// assert!(!AssignmentOp::Insert.is_set());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignmentOp {
    /// Set a field, replacing the current value.
    Set,

    /// Insert one or more values into a set field.
    Insert,

    /// Remove one or more values from a set field.
    Remove,
}

impl Statement {
    /// Returns this statement's assignments if it is an `Update`.
    pub fn assignments(&self) -> Option<&Assignments> {
        match self {
            Statement::Update(update) => Some(&update.assignments),
            _ => None,
        }
    }
}

impl Assignments {
    /// Creates an empty `Assignments` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            assignments: IndexMap::with_capacity(capacity),
            extra: Vec::new(),
        }
    }

    /// Returns `true` if there are no assignments.
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty() && self.extra.is_empty()
    }

    /// Returns the number of assignments (including extra entries for mixed
    /// operations on the same field).
    pub fn len(&self) -> usize {
        self.assignments.len() + self.extra.len()
    }

    /// Returns `true` if an assignment exists for the given projection.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.contains_key(key)
    }

    /// Returns a reference to the assignment for the given projection, if any.
    pub fn get<Q>(&self, key: &Q) -> Option<&Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.get(key)
    }

    /// Returns a mutable reference to the assignment for the given projection.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.get_mut(key)
    }

    /// Sets a field to the given expression value, replacing any existing
    /// assignment for that projection.
    pub fn set<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        self.assignments.insert(
            key,
            Assignment {
                op: AssignmentOp::Set,
                expr: expr.into(),
            },
        );
    }

    /// Removes all assignments for the given projection, including extra
    /// entries.
    pub fn unset<Q>(&mut self, key: &Q)
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.swap_remove(key);
        self.extra.retain(|(p, _)| !key.equivalent(p));
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    ///
    /// When there is already an `Insert` assignment for this projection, the
    /// expressions are merged into a list so that multiple values can be
    /// inserted in a single update.
    pub fn insert<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new_expr = expr.into();

        // If there is already an assignment for this projection, either merge
        // (same op) or overflow (different op).
        if let Some(existing) = self.assignments.get_mut(&key) {
            if existing.op == AssignmentOp::Insert {
                merge_expr(&mut existing.expr, new_expr);
                return;
            }
            // Different op (e.g., Remove already present) — store in extra.
            self.extra.push((
                key,
                Assignment {
                    op: AssignmentOp::Insert,
                    expr: new_expr,
                },
            ));
            return;
        }

        self.assignments.insert(
            key,
            Assignment {
                op: AssignmentOp::Insert,
                expr: new_expr,
            },
        );
    }

    /// Adds a `Remove` assignment for the given projection.
    ///
    /// When there is already a `Remove` assignment for this projection, the
    /// expressions are merged into a list so that multiple values can be
    /// removed in a single update.
    pub fn remove<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new_expr = expr.into();

        if let Some(existing) = self.assignments.get_mut(&key) {
            if existing.op == AssignmentOp::Remove {
                merge_expr(&mut existing.expr, new_expr);
                return;
            }
            // Different op — store in extra.
            self.extra.push((
                key,
                Assignment {
                    op: AssignmentOp::Remove,
                    expr: new_expr,
                },
            ));
            return;
        }

        self.assignments.insert(
            key,
            Assignment {
                op: AssignmentOp::Remove,
                expr: new_expr,
            },
        );
    }

    /// Removes and returns the assignment for the given projection.
    ///
    /// Returns only the primary entry. Use [`take_all`](Self::take_all) to
    /// retrieve both primary and extra entries (e.g., mixed insert + remove).
    pub fn take<Q>(&mut self, key: &Q) -> Option<Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.swap_remove(key)
    }

    /// Removes and returns **all** assignments for the given projection,
    /// including any extra entries with different operation types.
    ///
    /// This is needed for has-many fields that may have both insert and remove
    /// assignments in a single update.
    pub fn take_all<Q>(&mut self, key: &Q) -> Vec<Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        let mut result = Vec::new();

        if let Some(primary) = self.assignments.swap_remove(key) {
            result.push(primary);
        }

        // Drain matching entries from extra (in reverse to preserve order
        // while removing).
        let mut i = 0;
        while i < self.extra.len() {
            if key.equivalent(&self.extra[i].0) {
                result.push(self.extra.swap_remove(i).1);
            } else {
                i += 1;
            }
        }

        result
    }

    /// Returns an iterator over the assignment projections (keys).
    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.assignments.keys()
    }

    /// Returns an iterator over the assignment expressions (values), including
    /// extra entries.
    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.assignments
            .values()
            .map(|a| &a.expr)
            .chain(self.extra.iter().map(|(_, a)| &a.expr))
    }

    /// Returns an iterator over `(projection, assignment)` pairs, including
    /// extra entries for mixed operations.
    pub fn iter(&self) -> impl Iterator<Item = (&Projection, &Assignment)> + '_ {
        self.assignments
            .iter()
            .chain(self.extra.iter().map(|(p, a)| (p, a)))
    }

    /// Returns a mutable iterator over `(projection, assignment)` pairs,
    /// including extra entries for mixed operations.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Projection, &mut Assignment)> + '_ {
        self.assignments
            .iter_mut()
            .chain(self.extra.iter_mut().map(|(p, a)| (&*p, a)))
    }
}

impl IntoIterator for Assignments {
    type Item = (Projection, Assignment);
    type IntoIter = std::iter::Chain<
        indexmap::map::IntoIter<Projection, Assignment>,
        std::vec::IntoIter<(Projection, Assignment)>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.into_iter().chain(self.extra)
    }
}

impl Default for Assignments {
    fn default() -> Self {
        Self {
            assignments: IndexMap::new(),
            extra: Vec::new(),
        }
    }
}

impl<Q> ops::Index<Q> for Assignments
where
    Q: Hash + Equivalent<Projection>,
{
    type Output = Assignment;

    fn index(&self, index: Q) -> &Self::Output {
        match self.assignments.get(&index) {
            Some(ret) => ret,
            None => panic!("no assignment for projection"),
        }
    }
}

impl<Q> ops::IndexMut<Q> for Assignments
where
    Q: Hash + Equivalent<Projection>,
{
    fn index_mut(&mut self, index: Q) -> &mut Self::Output {
        match self.assignments.get_mut(&index) {
            Some(ret) => ret,
            None => panic!("no assignment for projection"),
        }
    }
}

/// Merge `new_expr` into `existing`. If `existing` is already a list, the new
/// expression is appended; otherwise both expressions are wrapped in a new list.
fn merge_expr(existing: &mut Expr, new_expr: Expr) {
    let old = std::mem::replace(existing, Expr::null());
    match old {
        Expr::List(mut list) => {
            list.items.push(new_expr);
            *existing = list.into();
        }
        other => {
            *existing = Expr::list_from_vec(vec![other, new_expr]);
        }
    }
}

impl AssignmentOp {
    /// Returns `true` if this is the `Set` operation.
    pub fn is_set(self) -> bool {
        matches!(self, Self::Set)
    }

    /// Returns `true` if this is the `Remove` operation.
    pub fn is_remove(self) -> bool {
        matches!(self, Self::Remove)
    }
}
