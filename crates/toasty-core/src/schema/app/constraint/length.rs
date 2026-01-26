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

        let value_len = value.len();
        let min = self.min.map(|m| m as usize);
        let max = self.max.map(|m| m as usize);

        // Check minimum length
        if let Some(min_val) = min {
            if value_len < min_val {
                return Err(crate::Error::validation_length(value_len, min, max));
            }
        }

        // Check maximum length
        if let Some(max_val) = max {
            if value_len > max_val {
                return Err(crate::Error::validation_length(value_len, min, max));
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
