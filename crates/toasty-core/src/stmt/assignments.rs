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
        }
    }

    /// Returns `true` if there are no assignments.
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    /// Returns the number of assignments.
    pub fn len(&self) -> usize {
        self.assignments.len()
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

    /// Removes the assignment for the given projection, if any.
    pub fn unset<Q>(&mut self, key: &Q)
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.swap_remove(key);
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    pub fn insert<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        use indexmap::map::Entry;

        let key = key.into();
        match self.assignments.entry(key) {
            Entry::Occupied(_) => {
                todo!()
            }
            Entry::Vacant(entry) => {
                entry.insert(Assignment {
                    op: AssignmentOp::Insert,
                    expr: expr.into(),
                });
            }
        }
    }

    /// Adds a `Remove` assignment for the given projection.
    pub fn remove<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        use indexmap::map::Entry;

        let key = key.into();
        match self.assignments.entry(key) {
            Entry::Occupied(_) => {
                todo!()
            }
            Entry::Vacant(entry) => {
                entry.insert(Assignment {
                    op: AssignmentOp::Remove,
                    expr: expr.into(),
                });
            }
        }
    }

    /// Removes and returns the assignment for the given projection.
    pub fn take<Q>(&mut self, key: &Q) -> Option<Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.swap_remove(key)
    }

    /// Returns an iterator over the assignment projections (keys).
    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.assignments.keys()
    }

    /// Returns an iterator over the assignment expressions (values).
    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.assignments.values().map(|assignment| &assignment.expr)
    }

    /// Returns an iterator over `(projection, assignment)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&Projection, &Assignment)> + '_ {
        self.assignments.iter()
    }

    /// Returns a mutable iterator over `(projection, assignment)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Projection, &mut Assignment)> + '_ {
        self.assignments.iter_mut()
    }
}

impl IntoIterator for Assignments {
    type Item = (Projection, Assignment);
    type IntoIter = indexmap::map::IntoIter<Projection, Assignment>;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.into_iter()
    }
}

impl<'a> IntoIterator for &'a Assignments {
    type Item = (&'a Projection, &'a Assignment);
    type IntoIter = indexmap::map::Iter<'a, Projection, Assignment>;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.iter()
    }
}

impl Default for Assignments {
    fn default() -> Self {
        Self {
            assignments: IndexMap::new(),
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
