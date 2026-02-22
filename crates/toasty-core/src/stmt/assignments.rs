use crate::stmt::Statement;

use super::{Expr, Projection};

use indexmap::{Equivalent, IndexMap};
use std::{hash::Hash, ops};

#[derive(Clone, Debug, PartialEq)]
pub struct Assignments {
    /// Map from UpdateTarget field projection to assignment for that field. The
    /// projection may reference an application-level model field or a lowered
    /// table column. Supports both single-step (e.g., [0]) and multi-step
    /// projections (e.g., [0, 1] for nested fields).
    assignments: IndexMap<Projection, Assignment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// Assignment operation
    pub op: AssignmentOp,

    /// Expression use for assignment
    pub expr: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignmentOp {
    /// Set a field, replacing the current value.
    Set,

    /// Insert one or more values into a set
    Insert,

    /// Remove one or more values from a set.
    Remove,
}

impl Statement {
    pub fn assignments(&self) -> Option<&Assignments> {
        match self {
            Statement::Update(update) => Some(&update.assignments),
            _ => None,
        }
    }
}

impl Assignments {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            assignments: IndexMap::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.contains_key(key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.get(key)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.get_mut(key)
    }

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

    pub fn take<Q>(&mut self, key: &Q) -> Option<Assignment>
    where
        Q: ?Sized + Hash + Equivalent<Projection>,
    {
        self.assignments.swap_remove(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &Projection> + '_ {
        self.assignments.keys()
    }

    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.assignments.values().map(|assignment| &assignment.expr)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Projection, &Assignment)> + '_ {
        self.assignments.iter()
    }

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
    pub fn is_set(self) -> bool {
        matches!(self, Self::Set)
    }

    pub fn is_remove(self) -> bool {
        matches!(self, Self::Remove)
    }
}
