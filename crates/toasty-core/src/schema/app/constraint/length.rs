use super::Constraint;

use crate::{Result, stmt};

/// A length constraint for string fields.
///
/// Enforces minimum and/or maximum character-count bounds on a string value.
/// Either bound can be `None` to leave that side unconstrained.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::ConstraintLength;
///
/// let c = ConstraintLength { min: Some(1), max: Some(255) };
/// assert_eq!(c.min, Some(1));
/// assert_eq!(c.max, Some(255));
/// ```
#[derive(Debug, Clone)]
pub struct ConstraintLength {
    /// The minimum length of the field. `None` means no lower bound.
    pub min: Option<u64>,

    /// The maximum length of the field. `None` means no upper bound.
    pub max: Option<u64>,
}
impl ConstraintLength {
    pub(crate) fn check(&self, expr: &crate::stmt::Entry<'_>) -> Result<()> {
        let Some(stmt::Value::String(value)) = expr.as_value() else {
            return Ok(());
        };

        let value_len = value.len();
        let min = self.min.map(|m| m as usize);
        let max = self.max.map(|m| m as usize);

        // Check minimum length
        if let Some(min_val) = min
            && value_len < min_val
        {
            return Err(crate::Error::validation_length(value_len, min, max));
        }

        // Check maximum length
        if let Some(max_val) = max
            && value_len > max_val
        {
            return Err(crate::Error::validation_length(value_len, min, max));
        }

        Ok(())
    }
}

impl From<ConstraintLength> for Constraint {
    fn from(length: ConstraintLength) -> Self {
        Constraint::Length(length)
    }
}
