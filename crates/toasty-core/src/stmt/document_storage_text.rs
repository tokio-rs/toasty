use std::fmt;

use crate::stmt::Value;

impl Value {
    /// The text form this value takes when rendered for document storage, or
    /// `None` if the value has no document text form.
    ///
    /// Values that are stored as JSON strings inside a `#[document]` column
    /// take this form: jiff temporal values (truncated to microseconds — the
    /// precision the SQL temporal types hold — and formatted with *fixed*
    /// six-digit subsecond precision) and decimals (their `Display` form).
    /// Fixed temporal precision matters on backends that compare document
    /// leaves as plain text (SQLite has no native temporal types, so
    /// `json_extract` comparisons are text comparisons): uniform-precision
    /// ISO 8601 strings sort lexicographically in chronological order, while
    /// trimmed subseconds do not (`...T00:00:00Z` sorts *after*
    /// `...T00:00:00.000001Z`).
    ///
    /// Both the JSON document codec (`toasty-sql`) and the engine's document
    /// lowering (which rewrites comparison operands to text on those
    /// backends) render document text through this one method, so the stored
    /// form and a bound comparison operand cannot drift apart.
    ///
    /// `Zoned` has no document text form: its RFC 9557 `[IANA]` annotation is
    /// rejected at schema build.
    pub fn document_storage_text(&self) -> Option<DocumentStorageText<'_>> {
        match self {
            #[cfg(feature = "jiff")]
            Value::Timestamp(_) | Value::Date(_) | Value::Time(_) | Value::DateTime(_) => {
                Some(DocumentStorageText(self))
            }
            #[cfg(feature = "rust_decimal")]
            Value::Decimal(_) => Some(DocumentStorageText(self)),
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(_) => Some(DocumentStorageText(self)),
            _ => None,
        }
    }
}

/// Helper struct for rendering a [`Value`]'s document storage text form.
///
/// Returned by [`Value::document_storage_text`]; see its documentation for
/// the format contract. Like [`std::path::Display`], this is an opaque
/// adapter — the only way to obtain one is the method that guarantees the
/// value has a document text form.
#[derive(Debug)]
pub struct DocumentStorageText<'a>(&'a Value);

impl fmt::Display for DocumentStorageText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            #[cfg(feature = "jiff")]
            Value::Timestamp(v) => write!(f, "{:.6}", trunc_timestamp_us(*v)),
            #[cfg(feature = "jiff")]
            Value::Date(v) => write!(f, "{v}"),
            #[cfg(feature = "jiff")]
            Value::Time(v) => write!(f, "{:.6}", trunc_time_us(*v)),
            #[cfg(feature = "jiff")]
            Value::DateTime(v) => write!(f, "{:.6}", trunc_datetime_us(*v)),
            #[cfg(feature = "rust_decimal")]
            Value::Decimal(v) => write!(f, "{v}"),
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(v) => write!(f, "{v}"),
            // `document_storage_text` only constructs the adapter for the
            // variants above.
            _ => unreachable!(),
        }
    }
}

/// Truncate a timestamp to microsecond precision, toward zero, dropping any
/// sub-microsecond nanoseconds. Rounding can only fail at the extreme ends of
/// the representable range; fall back to the original value there rather than
/// failing the whole encode.
#[cfg(feature = "jiff")]
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
#[cfg(feature = "jiff")]
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
#[cfg(feature = "jiff")]
fn trunc_datetime_us(v: jiff::civil::DateTime) -> jiff::civil::DateTime {
    v.round(
        jiff::civil::DateTimeRound::new()
            .smallest(jiff::Unit::Microsecond)
            .mode(jiff::RoundMode::Trunc),
    )
    .unwrap_or(v)
}
