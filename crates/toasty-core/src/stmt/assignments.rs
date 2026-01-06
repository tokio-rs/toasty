use crate::stmt::Statement;

use super::{Expr, Projection};

use indexmap::{Equivalent, IndexMap};
use std::{hash::Hash, ops};

#[derive(Clone, Debug, PartialEq)]
pub struct Assignments {
    /// Map from UpdateTarget field the assignment for that field. The
    /// UpdateTarget field may be an application-level model field or a lowered
    /// table column.
    assignments: IndexMap<usize, Assignment>,
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

    pub fn contains(&self, key: usize) -> bool {
        self.assignments.contains_key(&key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&Assignment>
    where
        Q: ?Sized + Hash + Equivalent<usize>,
    {
        self.assignments.get(key)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Assignment>
    where
        Q: ?Sized + Hash + Equivalent<usize>,
    {
        self.assignments.get_mut(key)
    }

    pub fn set<Q>(&mut self, key: Q, expr: impl Into<Expr>)
    where
        Q: Into<Projection>,
    {
        let key = key.into();
        let [key] = key.as_slice() else { todo!() };
        self.assignments.insert(
            *key,
            Assignment {
                op: AssignmentOp::Set,
                expr: expr.into(),
            },
        );
    }

    pub fn unset(&mut self, key: usize) {
        self.assignments.swap_remove(&key);
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    pub fn insert(&mut self, key: usize, expr: impl Into<Expr>) {
        use indexmap::map::Entry;

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

    pub fn remove(&mut self, key: usize, expr: impl Into<Expr>) {
        use indexmap::map::Entry;

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
        Q: ?Sized + Hash + Equivalent<usize>,
    {
        self.assignments.swap_remove(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = usize> + '_ {
        self.assignments.keys().copied()
    }

    pub fn exprs(&self) -> impl Iterator<Item = &Expr> + '_ {
        self.assignments.values().map(|assignment| &assignment.expr)
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Assignment)> + '_ {
        self.assignments
            .iter()
            .map(|(index, assignment)| (*index, assignment))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut Assignment)> + '_ {
        self.assignments
            .iter_mut()
            .map(|(index, assignment)| (*index, assignment))
    }
}

impl IntoIterator for Assignments {
    type Item = (usize, Assignment);

    type IntoIter = std::vec::IntoIter<(usize, Assignment)>;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.into_iter().collect::<Vec<_>>().into_iter()
    }
}

impl Default for Assignments {
    fn default() -> Self {
        Self {
            assignments: IndexMap::new(),
        }
    }
}

impl ops::Index<usize> for Assignments {
    type Output = Assignment;

    fn index(&self, index: usize) -> &Self::Output {
        match self.assignments.get(&index) {
            Some(ret) => ret,
            None => panic!("no assignment for field index {index}"),
        }
    }
}

impl ops::IndexMut<usize> for Assignments {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self.assignments.get_mut(&index) {
            Some(ret) => ret,
            None => panic!("no assignment for field index {index}"),
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
