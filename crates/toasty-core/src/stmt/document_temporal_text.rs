use std::fmt;

use crate::stmt::Value;

/// The text form a jiff temporal value takes inside a JSON document column.
///
/// Values are truncated to microseconds — the precision the SQL temporal
/// types hold — and formatted with *fixed* six-digit subsecond precision.
/// Fixed precision matters on backends that compare document leaves as plain
/// text (SQLite has no native temporal types, so `json_extract` comparisons
/// are text comparisons): uniform-precision ISO 8601 strings sort
/// lexicographically in chronological order, while trimmed subseconds do not
/// (`...T00:00:00Z` sorts *after* `...T00:00:00.000001Z`).
///
/// Both the JSON document codec (`toasty-sql`) and the engine's document
/// lowering (which rewrites temporal comparison operands to text on those
/// backends) build temporal text through this one type, so the stored form
/// and a bound comparison operand cannot drift apart.
#[derive(Debug, Clone, Copy)]
pub enum DocumentTemporalText {
    /// An instant, truncated to microseconds.
    Timestamp(jiff::Timestamp),
    /// A civil date (no sub-second component to normalize).
    Date(jiff::civil::Date),
    /// A civil time, truncated to microseconds.
    Time(jiff::civil::Time),
    /// A civil datetime, truncated to microseconds.
    DateTime(jiff::civil::DateTime),
}

impl DocumentTemporalText {
    /// The document text form of `value`, or `None` if it is not a temporal
    /// value with a document text form (`Zoned` has none: its RFC 9557
    /// `[IANA]` annotation is rejected at schema build).
    pub fn of(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Timestamp(v) => Self::Timestamp(trunc_timestamp_us(*v)),
            Value::Date(v) => Self::Date(*v),
            Value::Time(v) => Self::Time(trunc_time_us(*v)),
            Value::DateTime(v) => Self::DateTime(trunc_datetime_us(*v)),
            _ => return None,
        })
    }
}

impl fmt::Display for DocumentTemporalText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timestamp(v) => write!(f, "{v:.6}"),
            Self::Date(v) => write!(f, "{v}"),
            Self::Time(v) => write!(f, "{v:.6}"),
            Self::DateTime(v) => write!(f, "{v:.6}"),
        }
    }
}

/// Truncate a timestamp to microsecond precision, toward zero, dropping any
/// sub-microsecond nanoseconds. Rounding can only fail at the extreme ends of
/// the representable range; fall back to the original value there rather than
/// failing the whole encode.
fn trunc_timestamp_us(v: jiff::Timestamp) -> jiff::Timestamp {
    v.round(
        jiff::TimestampRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}

/// Truncate a civil time to microsecond precision, toward zero. See
/// [`trunc_timestamp_us`].
fn trunc_time_us(v: jiff::civil::Time) -> jiff::civil::Time {
    v.round(
        jiff::civil::TimeRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}

/// Truncate a civil datetime to microsecond precision, toward zero. See
/// [`trunc_timestamp_us`].
fn trunc_datetime_us(v: jiff::civil::DateTime) -> jiff::civil::DateTime {
    v.round(
        jiff::civil::DateTimeRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}
