/// Strategy for automatically populating a field's value on insert.
///
/// When a field has an `AutoStrategy`, Toasty generates the value
/// automatically when a new record is created, rather than requiring the
/// caller to supply it.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{AutoStrategy, UuidVersion};
///
/// let strategy = AutoStrategy::Uuid(UuidVersion::V4);
/// assert!(!strategy.is_increment());
///
/// let inc = AutoStrategy::Increment;
/// assert!(inc.is_increment());
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AutoStrategy {
    /// Generate a UUID of the specified version.
    Uuid(UuidVersion),
    /// Use an auto-incrementing integer sequence (database-assigned).
    Increment,
}

/// UUID version to use for auto-generated UUID fields.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::UuidVersion;
///
/// let v4 = UuidVersion::V4;
/// let v7 = UuidVersion::V7;
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UuidVersion {
    /// Random UUID (version 4).
    V4,
    /// Time-ordered UUID (version 7).
    V7,
}

impl AutoStrategy {
    /// Returns `true` if this strategy is [`AutoStrategy::Increment`].
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::app::AutoStrategy;
    ///
    /// let inc = AutoStrategy::Increment;
    /// assert!(inc.is_increment());
    /// ```
    #[must_use]
    pub fn is_increment(&self) -> bool {
        matches!(self, Self::Increment)
    }
}
