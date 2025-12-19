/// How toasty should populate the field
#[derive(Debug, Clone)]
pub enum AutoStrategy {
    Id,
    Uuid(UuidVersion),
    Increment,
}

#[derive(Debug, Clone)]
pub enum UuidVersion {
    V4,
    V7,
}

impl AutoStrategy {
    /// Returns `true` if the auto is [`Increment`].
    ///
    /// [`Increment`]: Auto::Increment
    #[must_use]
    pub fn is_increment(&self) -> bool {
        matches!(self, Self::Increment)
    }
}
