mod length;
pub use length::ConstraintLength;

use crate::{Result, stmt};

/// A validation constraint applied to a field.
///
/// Constraints are checked at the application level before values are persisted
/// to the database. If a constraint check fails, the operation returns an error
/// instead of sending the value to the driver.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::Constraint;
///
/// let c = Constraint::length_less_than(255);
/// // The constraint can be checked against field values at runtime.
/// ```
#[derive(Debug, Clone)]
pub enum Constraint {
    /// A length constraint on a string field.
    Length(ConstraintLength),
}

impl Constraint {
    /// Creates a length constraint requiring the value to be shorter than `max`
    /// characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::app::Constraint;
    ///
    /// let c = Constraint::length_less_than(100);
    /// ```
    pub fn length_less_than(max: u64) -> Self {
        ConstraintLength {
            min: None,
            max: Some(max),
        }
        .into()
    }

    /// Validates `expr` against this constraint.
    ///
    /// Returns `Ok(())` if the value satisfies the constraint, or an error
    /// describing the violation.
    pub fn check(&self, expr: &stmt::Entry<'_>) -> Result<()> {
        match self {
            Constraint::Length(length) => length.check(expr),
        }
    }
}
