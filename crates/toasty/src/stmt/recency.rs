/// Marker trait for types that can be meaningfully ordered by recency.
///
/// Only time-based types implement this trait, preventing nonsensical
/// ordering at compile time. Implement this trait on a type to allow
/// it to be used with [`Query::latest_by`].
pub trait Recency {}

#[cfg(feature = "jiff")]
impl Recency for jiff::Timestamp {}
#[cfg(feature = "jiff")]
impl Recency for jiff::Zoned {}
#[cfg(feature = "jiff")]
impl Recency for jiff::civil::DateTime {}
#[cfg(feature = "jiff")]
impl Recency for jiff::civil::Date {}
