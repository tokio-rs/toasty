use crate::schema::Index;

use super::*;

use indexmap::IndexMap;
use std::ops;

#[derive(Clone, PartialEq, Debug)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentOp {
    /// Set a field, replacing the current value.
    Set,

    /// Insert one or more values into a set
    Insert,

    /// Remove one or more values from a set.
    Remove,
}

impl Assignments {
    pub fn with_capacity(capacity: usize) -> Assignments {
        Assignments {
            assignments: IndexMap::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    pub fn contains(&self, key: impl Into<PathStep>) -> bool {
        self.assignments.contains_key(&key.into().into_usize())
    }

    pub fn get(&self, key: impl Into<PathStep>) -> Option<&Assignment> {
        let index = key.into().into_usize();
        self.assignments.get(&index)
    }

    pub fn set(&mut self, key: impl Into<PathStep>, expr: impl Into<Expr>) {
        self.assignments.insert(
            key.into().into_usize(),
            Assignment {
                op: AssignmentOp::Set,
                expr: expr.into(),
            },
        );
    }

    pub fn unset(&mut self, key: impl Into<PathStep>) {
        self.assignments.swap_remove(&key.into().into_usize());
    }

    /// Insert a value into a set. The expression should evaluate to a single
    /// value that is inserted into the set.
    pub fn insert(&mut self, key: impl Into<PathStep>, expr: impl Into<Expr>) {
        use indexmap::map::Entry;

        match self.assignments.entry(key.into().into_usize()) {
            Entry::Occupied(entry) => {
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

    pub fn take(&mut self, key: impl Into<PathStep>) -> Assignment {
        self.assignments
            .swap_remove(&key.into().into_usize())
            .unwrap()
    }

    pub fn keys(&self) -> impl Iterator<Item = usize> + '_ {
        self.assignments.keys().map(|k| *k)
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

impl Default for Assignments {
    fn default() -> Self {
        Assignments {
            assignments: IndexMap::new(),
        }
    }
}

impl<I: Into<PathStep>> ops::Index<I> for Assignments {
    type Output = Expr;

    fn index(&self, index: I) -> &Self::Output {
        let index = index.into().into_usize();
        &self.assignments.get(&index).unwrap().expr
    }
}

impl<I: Into<PathStep>> ops::IndexMut<I> for Assignments {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let index = index.into().into_usize();
        &mut self.assignments.get_mut(&index).unwrap().expr
    }
}
