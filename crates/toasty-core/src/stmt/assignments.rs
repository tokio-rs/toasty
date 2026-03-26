use crate::stmt::Statement;

use super::{AsProjection, Expr, Projection};

use std::collections::BTreeMap;

/// An ordered map of field assignments for an [`Update`](super::Update) statement.
///
/// Entries are sorted by projection (lexicographic on the step sequence), so
/// prefix-range queries work naturally: to find every assignment under field 1
/// (including `[1]`, `[1, 0]`, `[1, 2]`, …), use
/// [`range`](Self::range)`(Projection::single(1)..Projection::single(2))`.
///
/// Each projection may have multiple assignments with different operation types
/// (e.g., both an `Insert` and a `Remove` for a has-many field). Same-op
/// entries are merged into list expressions automatically.
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
    /// Sorted map from projection to one or more assignments. Multiple entries
    /// per projection arise when different operation types (e.g., Insert +
    /// Remove) target the same field.
    inner: BTreeMap<Projection, Vec<Assignment>>,
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
        Self::default()
    }

    /// Returns `true` if there are no assignments.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the total number of individual assignments across all
    /// projections. A projection with both an `Insert` and a `Remove` counts
    /// as two.
    pub fn len(&self) -> usize {
        self.inner.values().map(|v| v.len()).sum()
    }

    /// Returns `true` if at least one assignment exists for the given
    /// projection.
    pub fn contains(&self, key: &Projection) -> bool {
        self.inner.contains_key(key)
    }

    /// Returns a reference to the first assignment for the given projection.
    ///
    /// Also accepts `&usize` for single-field lookups via the
    /// [`AsProjection`](super::AsProjection) trait.
    pub fn get<Q: AsProjection>(&self, key: &Q) -> Option<&Assignment> {
        self.inner.get(&key.as_projection()).and_then(|v| v.first())
    }

    /// Returns a mutable reference to the first assignment for the given
    /// projection.
    pub fn get_mut<Q: AsProjection>(&mut self, key: &Q) -> Option<&mut Assignment> {
        self.inner
            .get_mut(&key.as_projection())
            .and_then(|v| v.first_mut())
    }

    /// Sets a field to the given expression value, replacing **all** existing
    /// assignments for that projection.
    pub fn set(&mut self, key: impl Into<Projection>, expr: impl Into<Expr>) {
        let key = key.into();
        self.inner.insert(
            key,
            vec![Assignment {
                op: AssignmentOp::Set,
                expr: expr.into(),
            }],
        );
    }

    /// Removes all assignments for the given projection.
    pub fn unset(&mut self, key: &Projection) {
        self.inner.remove(key);
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    ///
    /// When there is already an `Insert` assignment for this projection, the
    /// expressions are merged into a list so that multiple values can be
    /// inserted in a single update. A different op (e.g., `Remove`) coexists
    /// as a separate entry.
    pub fn insert(&mut self, key: impl Into<Projection>, expr: impl Into<Expr>) {
        let key = key.into();
        let new_expr = expr.into();
        push_or_merge(&mut self.inner, key, AssignmentOp::Insert, new_expr);
    }

    /// Adds a `Remove` assignment for the given projection.
    ///
    /// When there is already a `Remove` assignment for this projection, the
    /// expressions are merged into a list so that multiple values can be
    /// removed in a single update. A different op coexists as a separate entry.
    pub fn remove(&mut self, key: impl Into<Projection>, expr: impl Into<Expr>) {
        let key = key.into();
        let new_expr = expr.into();
        push_or_merge(&mut self.inner, key, AssignmentOp::Remove, new_expr);
    }

    /// Removes and returns the first assignment for the given projection.
    ///
    /// If the projection has only one assignment, the entry is removed
    /// entirely. Otherwise, the first assignment is popped and the rest remain.
    ///
    /// Use [`take_all`](Self::take_all) to retrieve every assignment for a
    /// projection (e.g., mixed insert + remove).
    pub fn take(&mut self, key: &Projection) -> Option<Assignment> {
        let entries = self.inner.get_mut(key)?;

        if entries.len() == 1 {
            // Remove the whole key when the last entry is taken.
            self.inner.remove(key).unwrap().into_iter().next()
        } else {
            // Pop the first entry; the rest stay.
            Some(entries.remove(0))
        }
    }

    /// Removes and returns **all** assignments for the given projection.
    pub fn take_all(&mut self, key: &Projection) -> Vec<Assignment> {
        self.inner.remove(key).unwrap_or_default()
    }

    /// Returns an iterator over the unique assignment projections (keys),
    /// in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.inner.keys()
    }

    /// Returns an iterator over all assignment expressions (values).
    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.inner.values().flat_map(|v| v.iter()).map(|a| &a.expr)
    }

    /// Returns an iterator over all `(projection, assignment)` pairs.
    ///
    /// Projections with multiple assignments yield one pair per assignment.
    pub fn iter(&self) -> impl Iterator<Item = (&Projection, &Assignment)> + '_ {
        self.inner
            .iter()
            .flat_map(|(p, v)| v.iter().map(move |a| (p, a)))
    }

    /// Returns a mutable iterator over all `(projection, assignment)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Projection, &mut Assignment)> + '_ {
        self.inner
            .iter_mut()
            .flat_map(|(p, v)| v.iter_mut().map(move |a| (p as &Projection, a)))
    }

    /// Returns an iterator over `(projection, assignments)` pairs whose
    /// projections fall within the given range.
    ///
    /// This enables prefix queries: to find every assignment under field 1
    /// (including `[1]`, `[1, 0]`, `[1, 2]`, …):
    ///
    /// ```ignore
    /// let field_1 = Projection::single(1);
    /// let field_2 = Projection::single(2);
    /// for (proj, assignments) in assignments.range(field_1..field_2) {
    ///     // ...
    /// }
    /// ```
    pub fn range<R>(&self, range: R) -> impl Iterator<Item = (&Projection, &[Assignment])> + '_
    where
        R: std::ops::RangeBounds<Projection>,
    {
        self.inner.range(range).map(|(p, v)| (p, v.as_slice()))
    }
}

/// If the vec already has an entry with the same op, merge the expression into
/// it (turning it into a list if needed). Otherwise, push a new entry.
fn push_or_merge(
    map: &mut BTreeMap<Projection, Vec<Assignment>>,
    key: Projection,
    op: AssignmentOp,
    new_expr: Expr,
) {
    let entries = map.entry(key).or_default();

    // Look for an existing entry with the same op to merge into.
    if let Some(existing) = entries.iter_mut().find(|a| a.op == op) {
        merge_expr(&mut existing.expr, new_expr);
    } else {
        entries.push(Assignment { op, expr: new_expr });
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

impl IntoIterator for Assignments {
    type Item = (Projection, Assignment);
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.inner.into_iter(),
            current: None,
        }
    }
}

/// Owning iterator that flattens `BTreeMap<Projection, Vec<Assignment>>` into
/// `(Projection, Assignment)` pairs.
pub struct IntoIter {
    inner: std::collections::btree_map::IntoIter<Projection, Vec<Assignment>>,
    current: Option<(Projection, std::vec::IntoIter<Assignment>)>,
}

impl Iterator for IntoIter {
    type Item = (Projection, Assignment);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((ref proj, ref mut vec_iter)) = self.current {
                if let Some(assignment) = vec_iter.next() {
                    return Some((proj.clone(), assignment));
                }
            }
            // Advance to the next key.
            let (proj, vec) = self.inner.next()?;
            self.current = Some((proj, vec.into_iter()));
        }
    }
}

// --- Index impls --------------------------------------------------------
//
// These allow `assignments[usize]` and `assignments[Projection]` to retrieve
// the first assignment for a given projection, matching the pre-refactor API.

impl std::ops::Index<usize> for Assignments {
    type Output = Assignment;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(&index).expect("no assignment for projection")
    }
}

impl std::ops::IndexMut<usize> for Assignments {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(&index).expect("no assignment for projection")
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
