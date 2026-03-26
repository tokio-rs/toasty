use crate::stmt::Statement;

use super::{Expr, Projection};

use std::{collections::BTreeMap, ops};

/// An ordered map of field assignments for an [`Update`](super::Update) statement.
///
/// Each entry maps a field projection (identifying which field to change) to one
/// or more [`Assignment`] entries (how to change it). The entries are ordered by
/// projection, allowing range queries over prefixes.
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assignments {
    /// Map from field projection to assignment(s). The projection may reference an
    /// application-level model field or a lowered table column. Supports both
    /// single-step (e.g., `[0]`) and multi-step projections (e.g., `[0, 1]`
    /// for nested fields).
    ///
    /// Each key maps to a `Vec<Assignment>` to support multiple operations on
    /// the same projection (e.g., multiple inserts into a set field).
    assignments: BTreeMap<Projection, Vec<Assignment>>,
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
    /// Creates an empty `Assignments`.
    pub fn new() -> Self {
        Self {
            assignments: BTreeMap::new(),
        }
    }

    /// Returns `true` if there are no assignments.
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    /// Returns the number of assigned projections (keys).
    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    /// Returns `true` if an assignment exists for the given projection.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.contains_key(key.as_ref())
    }

    /// Returns the first assignment for the given projection, if any.
    ///
    /// When multiple assignments exist for a key, this returns the first one.
    pub fn get<Q>(&self, key: &Q) -> Option<&Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.get(key.as_ref()).and_then(|v| v.first())
    }

    /// Returns all assignments for the given projection.
    pub fn get_all<Q>(&self, key: &Q) -> Option<&[Assignment]>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.get(key.as_ref()).map(|v| v.as_slice())
    }

    /// Returns a mutable reference to the first assignment for the given projection.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments
            .get_mut(key.as_ref())
            .and_then(|v| v.first_mut())
    }

    /// Sets a field to the given expression value, replacing any existing
    /// assignments for that projection.
    pub fn set<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        self.assignments.insert(
            key,
            vec![Assignment {
                op: AssignmentOp::Set,
                expr: expr.into(),
            }],
        );
    }

    /// Removes the assignment for the given projection, if any.
    pub fn unset<Q>(&mut self, key: &Q)
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.remove(key.as_ref());
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    pub fn insert<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        self.assignments.entry(key).or_default().push(Assignment {
            op: AssignmentOp::Insert,
            expr: expr.into(),
        });
    }

    /// Adds a `Remove` assignment for the given projection.
    pub fn remove<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        self.assignments.entry(key).or_default().push(Assignment {
            op: AssignmentOp::Remove,
            expr: expr.into(),
        });
    }

    /// Removes and returns the first assignment for the given projection.
    pub fn take<Q>(&mut self, key: &Q) -> Option<Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        let entries = self.assignments.remove(key.as_ref())?;
        entries.into_iter().next()
    }

    /// Returns an iterator over the assignment projections (keys).
    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.assignments.keys()
    }

    /// Returns an iterator over the assignment expressions (values).
    ///
    /// When a key has multiple assignments, all expressions are yielded.
    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.assignments
            .values()
            .flat_map(|v| v.iter().map(|a| &a.expr))
    }

    /// Returns an iterator over `(projection, assignment)` pairs.
    ///
    /// When a key has multiple assignments, each is yielded as a separate pair.
    pub fn iter(&self) -> impl Iterator<Item = (&Projection, &Assignment)> + '_ {
        self.assignments
            .iter()
            .flat_map(|(k, v)| v.iter().map(move |a| (k, a)))
    }

    /// Returns a mutable iterator over `(projection, assignment)` pairs.
    ///
    /// When a key has multiple assignments, each is yielded as a separate pair.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Projection, &mut Assignment)> + '_ {
        self.assignments
            .iter_mut()
            .flat_map(|(k, v)| v.iter_mut().map(move |a| (k, a)))
    }
}

impl IntoIterator for Assignments {
    type Item = (Projection, Assignment);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.assignments.into_iter(),
            current: None,
        }
    }
}

/// Owning iterator over `(Projection, Assignment)` pairs.
pub struct IntoIter {
    inner: std::collections::btree_map::IntoIter<Projection, Vec<Assignment>>,
    current: Option<(Projection, std::vec::IntoIter<Assignment>)>,
}

impl Iterator for IntoIter {
    type Item = (Projection, Assignment);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((ref key, ref mut vec_iter)) = self.current {
                if let Some(assignment) = vec_iter.next() {
                    return Some((key.clone(), assignment));
                }
            }
            let (key, vec) = self.inner.next()?;
            self.current = Some((key, vec.into_iter()));
        }
    }
}

impl<'a> IntoIterator for &'a Assignments {
    type Item = (&'a Projection, &'a Assignment);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.assignments.iter(),
            current: None,
        }
    }
}

/// Borrowing iterator over `(&Projection, &Assignment)` pairs.
pub struct Iter<'a> {
    inner: std::collections::btree_map::Iter<'a, Projection, Vec<Assignment>>,
    current: Option<(&'a Projection, std::slice::Iter<'a, Assignment>)>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Projection, &'a Assignment);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((key, ref mut slice_iter)) = self.current {
                if let Some(assignment) = slice_iter.next() {
                    return Some((key, assignment));
                }
            }
            let (key, vec) = self.inner.next()?;
            self.current = Some((key, vec.iter()));
        }
    }
}

impl<Q> ops::Index<Q> for Assignments
where
    Q: Into<Projection>,
{
    type Output = Assignment;

    fn index(&self, index: Q) -> &Self::Output {
        let proj = index.into();
        match self.assignments.get(&proj).and_then(|v| v.first()) {
            Some(ret) => ret,
            None => panic!("no assignment for projection"),
        }
    }
}

impl<Q> ops::IndexMut<Q> for Assignments
where
    Q: Into<Projection> + Clone,
{
    fn index_mut(&mut self, index: Q) -> &mut Self::Output {
        let proj = index.into();
        match self.assignments.get_mut(&proj).and_then(|v| v.first_mut()) {
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
