use super::Constraint;

use crate::{stmt, Result};

#[derive(Debug, Clone)]
pub struct ConstraintLength {
    /// The minimum length of the field.
    pub min: Option<u64>,

    /// The maximum length of the field.
    pub max: Option<u64>,
}
impl ConstraintLength {
    pub(crate) fn check(&self, expr: &crate::stmt::Entry<'_>) -> Result<()> {
        let Some(stmt::Value::String(value)) = expr.try_as_value() else {
            return Ok(());
        };

        if let Some(min) = self.min {
            let min = min as usize;

            if value.len() < min {
                if self.max == self.min {
                    crate::bail!("value is too short; expected length of {min}");
                }

                crate::bail!("value is too short: {value:?} < {min}");
            }
        }

        if let Some(max) = self.max {
            let max = max as usize;

            if value.len() > max {
                crate::bail!("value is too long: {value:?} > {max}");
            }
        }

        Ok(())
    }
}

impl From<ConstraintLength> for Constraint {
    fn from(length: ConstraintLength) -> Self {
        Constraint::Length(length)
    }
}
