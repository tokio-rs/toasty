mod length;
pub use length::ConstraintLength;

use crate::{stmt, Result};

#[derive(Debug, Clone)]
pub enum Constraint {
    Length(ConstraintLength),
}

impl Constraint {
    pub fn length_less_than(max: u64) -> Self {
        ConstraintLength {
            min: None,
            max: Some(max),
        }
        .into()
    }

    pub fn check(&self, expr: &stmt::Entry<'_>) -> Result<()> {
        match self {
            Constraint::Length(length) => length.check(expr),
        }
    }
}
