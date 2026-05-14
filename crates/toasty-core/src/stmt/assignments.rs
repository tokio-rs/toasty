use crate::stmt::{Node, Statement, Visit, VisitMut};

use super::{Expr, Projection};

use std::{collections::BTreeMap, ops};

/// An ordered map of field assignments for an [`Update`](super::Update) statement.
///
/// Each entry maps a field projection (identifying which field to change) to an
/// [`Assignment`] (how to change it). The entries are ordered by projection,
/// allowing range queries over prefixes.
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
    /// Map from field projection to assignment. The projection may reference an
    /// application-level model field or a lowered table column. Supports both
    /// single-step (e.g., `[0]`) and multi-step projections (e.g., `[0, 1]`
    /// for nested fields).
    assignments: BTreeMap<Projection, Assignment>,
}

/// A field assignment within an [`Update`](super::Update) statement.
///
/// Each variant carries the expression providing the value for the operation.
/// Multiple operations on the same field are represented via [`Batch`](Assignment::Batch).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Assignment, Expr};
///
/// let assignment = Assignment::Set(Expr::null());
/// assert!(assignment.is_set());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Assignment {
    /// Set a field, replacing the current value.
    Set(Expr),

    /// Insert one or more values into a set field.
    Insert(Expr),

    /// Remove one or more values from a set field.
    Remove(Expr),

    /// Append every element of a list expression to the end of an ordered
    /// collection field (e.g. `Vec<scalar>`). The expression must evaluate
    /// to a list whose element type matches the target column.
    Append(Expr),

    /// Drop the last element of an ordered collection field. Drives
    /// [`stmt::pop`](crate::stmt::Assignment::Pop) on `Vec<scalar>` fields.
    /// No-op on an empty collection.
    Pop,

    /// Drop the element at the given index from an ordered collection field.
    /// The expression must evaluate to a `usize`-shaped value. Out-of-bounds
    /// indices are a no-op rather than an error — per-row failure semantics
    /// on a bulk update are rarely useful.
    RemoveAt(Expr),

    /// Multiple assignments on the same field.
    Batch(Vec<Assignment>),
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
    ///
    /// The `key` accepts any type that implements `AsRef<[usize]>`:
    /// - [`Projection`] — look up by projection directly
    /// - `&[usize]` — a slice of field indices (e.g., `&[1, 2]`)
    /// - `[usize; N]` — a fixed-size array (e.g., `[1]`, `[1, 2]`).
    ///   Integer literals infer as `usize` from the `AsRef<[usize]>` bound,
    ///   so `&[1]` works without a suffix.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.contains_key(key.as_ref())
    }

    /// Returns a reference to the assignment for the given projection, if any.
    ///
    /// The `key` accepts any type that implements `AsRef<[usize]>`:
    /// - [`Projection`] — look up by projection directly
    /// - `&[usize]` — a slice of field indices (e.g., `&[1, 2]`)
    /// - `[usize; N]` — a fixed-size array (e.g., `[1]`, `[1, 2]`).
    ///   Integer literals infer as `usize` from the `AsRef<[usize]>` bound,
    ///   so `&[1]` works without a suffix.
    pub fn get<Q>(&self, key: &Q) -> Option<&Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.get(key.as_ref())
    }

    /// Returns a mutable reference to the assignment for the given projection.
    ///
    /// The `key` accepts any type that implements `AsRef<[usize]>`:
    /// - [`Projection`] — look up by projection directly
    /// - `&[usize]` — a slice of field indices (e.g., `&[1, 2]`)
    /// - `[usize; N]` — a fixed-size array (e.g., `[1]`, `[1, 2]`).
    ///   Integer literals infer as `usize` from the `AsRef<[usize]>` bound,
    ///   so `&[1]` works without a suffix.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.get_mut(key.as_ref())
    }

    /// Sets a field to the given expression value, replacing any existing
    /// assignment for that projection.
    pub fn set<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        self.assignments.insert(key, Assignment::Set(expr.into()));
    }

    /// Removes the assignment for the given projection, if any.
    ///
    /// The `key` accepts any type that implements `AsRef<[usize]>`:
    /// - [`Projection`] — look up by projection directly
    /// - `&[usize]` — a slice of field indices (e.g., `&[1, 2]`)
    /// - `[usize; N]` — a fixed-size array (e.g., `[1]`, `[1, 2]`).
    ///   Integer literals infer as `usize` from the `AsRef<[usize]>` bound,
    ///   so `&[1]` works without a suffix.
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
        let new = Assignment::Insert(expr.into());
        self.assignments
            .entry(key)
            .and_modify(|existing| existing.push(new.clone()))
            .or_insert(new);
    }

    /// Adds a `Remove` assignment for the given projection.
    pub fn remove<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new = Assignment::Remove(expr.into());
        self.assignments
            .entry(key)
            .and_modify(|existing| existing.push(new.clone()))
            .or_insert(new);
    }

    /// Adds an `Append` assignment for the given projection. The expression
    /// should evaluate to a list of elements to append to an ordered
    /// collection field; multiple appends on the same projection batch.
    pub fn append<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new = Assignment::Append(expr.into());
        self.assignments
            .entry(key)
            .and_modify(|existing| existing.push(new.clone()))
            .or_insert(new);
    }

    /// Adds a `Pop` assignment for the given projection. Multiple pops on the
    /// same projection batch.
    pub fn pop<Q>(&mut self, key: Q)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new = Assignment::Pop;
        self.assignments
            .entry(key)
            .and_modify(|existing| existing.push(new.clone()))
            .or_insert(new);
    }

    /// Adds a `RemoveAt` assignment for the given projection. The expression
    /// should evaluate to the element index to drop. Multiple removals on the
    /// same projection batch.
    pub fn remove_at<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let new = Assignment::RemoveAt(expr.into());
        self.assignments
            .entry(key)
            .and_modify(|existing| existing.push(new.clone()))
            .or_insert(new);
    }

    /// Removes and returns the assignment for the given projection.
    ///
    /// The `key` accepts any type that implements `AsRef<[usize]>`:
    /// - [`Projection`] — look up by projection directly
    /// - `&[usize]` — a slice of field indices (e.g., `&[1, 2]`)
    /// - `[usize; N]` — a fixed-size array (e.g., `[1]`, `[1, 2]`).
    ///   Integer literals infer as `usize` from the `AsRef<[usize]>` bound,
    ///   so `&[1]` works without a suffix.
    pub fn take<Q>(&mut self, key: &Q) -> Option<Assignment>
    where
        Q: ?Sized + AsRef<[usize]>,
    {
        self.assignments.remove(key.as_ref())
    }

    /// Returns an iterator over the assignment projections (keys).
    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.assignments.keys()
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
    type IntoIter = std::collections::btree_map::IntoIter<Projection, Assignment>;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.into_iter()
    }
}

impl<'a> IntoIterator for &'a Assignments {
    type Item = (&'a Projection, &'a Assignment);
    type IntoIter = std::collections::btree_map::Iter<'a, Projection, Assignment>;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.iter()
    }
}

/// Indexes into the assignments by projection. Panics if no assignment exists
/// for the given key.
///
/// The index accepts any type that implements `AsRef<[usize]>`:
/// [`Projection`], `&[usize]`, or `[usize; N]` arrays.
impl<Q> ops::Index<Q> for Assignments
where
    Q: AsRef<[usize]>,
{
    type Output = Assignment;

    fn index(&self, index: Q) -> &Self::Output {
        match self.assignments.get(index.as_ref()) {
            Some(ret) => ret,
            None => panic!("no assignment for projection"),
        }
    }
}

/// Mutably indexes into the assignments by projection. Panics if no assignment
/// exists for the given key.
///
/// The index accepts any type that implements `AsRef<[usize]>`:
/// [`Projection`], `&[usize]`, or `[usize; N]` arrays.
impl<Q> ops::IndexMut<Q> for Assignments
where
    Q: AsRef<[usize]>,
{
    fn index_mut(&mut self, index: Q) -> &mut Self::Output {
        match self.assignments.get_mut(index.as_ref()) {
            Some(ret) => ret,
            None => panic!("no assignment for projection"),
        }
    }
}

impl Assignment {
    /// Returns `true` if this is the `Set` variant.
    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    /// Returns `true` if this is the `Remove` variant.
    pub fn is_remove(&self) -> bool {
        matches!(self, Self::Remove(_))
    }

    /// Appends another assignment, converting to `Batch` if needed.
    pub fn push(&mut self, other: Assignment) {
        match self {
            Self::Batch(entries) => entries.push(other),
            _ => {
                let prev = std::mem::replace(self, Assignment::Batch(Vec::new()));
                if let Assignment::Batch(entries) = self {
                    entries.push(prev);
                    entries.push(other);
                }
            }
        }
    }
}

impl Node for Assignment {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_assignment(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_assignment_mut(self);
    }
}
