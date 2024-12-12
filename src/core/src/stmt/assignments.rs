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
        // *self.slot(field.into().into_usize()) = expr.into();
        todo!()
    }

    pub fn unset(&mut self, key: impl Into<PathStep>) {
        /*
        let field = field.into();
        self.fields.unset(field);

        self.exprs[field.into_usize()] = None;
        */
        todo!()
    }

    pub fn push(&mut self, key: impl Into<PathStep>, expr: impl Into<Expr>) {
        // self.slot(field.into().into_usize()).push(expr.into());
        todo!()
    }

    pub fn take(&mut self, key: impl Into<PathStep>) -> stmt::Expr {
        /*
        let field = field.into();
        self.fields.unset(field);

        self.exprs[field.into_usize()].take().unwrap()
        */
        todo!()
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

    fn slot(&mut self, index: usize) -> &mut Expr {
        /*
        self.fields.insert(index);

        if self.exprs.len() <= index {
            self.exprs.resize(index + 1, None);
        }

        if self.exprs[index].is_none() {
            self.exprs[index] = Some(Expr::default());
        }

        self.exprs[index].as_mut().unwrap()
        */
        todo!()
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
        &self.assignments[index].expr
    }
}

impl<I: Into<PathStep>> ops::IndexMut<I> for Assignments {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let index = index.into().into_usize();
        &mut self.assignments[index].expr
    }
}
