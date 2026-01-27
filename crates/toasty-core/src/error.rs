mod adhoc;
mod condition_failed;
mod connection_pool;
mod driver;
mod invalid_result;
mod record_not_found;
mod too_many_records;
mod type_conversion;
mod validation;

use adhoc::AdhocError;
use condition_failed::ConditionFailedError;
use connection_pool::ConnectionPoolError;
use driver::DriverError;
use invalid_result::InvalidResultError;
use record_not_found::RecordNotFoundError;
use std::sync::Arc;
use too_many_records::TooManyRecordsError;
use type_conversion::TypeConversionError;
use validation::ValidationError;

/// Temporary helper macro during migration from anyhow.
///
/// This wraps `anyhow::bail!` and converts the result to our Error type.
/// Once we have structured errors, we'll replace uses of this macro with
/// proper error types.
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::from_args(format_args!($($arg)*)))
    };
}

/// Temporary helper macro for creating errors during migration from anyhow.
///
/// This wraps `anyhow::anyhow!` and converts to our Error type.
/// Once we have structured errors, we'll replace uses of this macro with
/// proper error types.
#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        $crate::Error::from_args(format_args!($($arg)*))
    };
}

/// An error that can occur in Toasty.
#[derive(Clone)]
pub struct Error {
    inner: Option<Arc<ErrorInner>>,
}

#[derive(Debug)]
struct ErrorInner {
    kind: ErrorKind,
    cause: Option<Error>,
}

impl Error {
    /// Adds context to this error.
    ///
    /// Context is displayed in reverse order: the most recently added context is shown first,
    /// followed by earlier context, ending with the root cause.
    #[inline(always)]
    pub fn context(self, consequent: impl IntoError) -> Error {
        self.context_impl(consequent.into_error())
    }

    #[inline(never)]
    #[cold]
    fn context_impl(self, consequent: Error) -> Error {
        let mut err = consequent;
        if err.inner.is_none() {
            err = Error::from(ErrorKind::Unknown);
        }
        let inner = err.inner.as_mut().unwrap();
        assert!(
            inner.cause.is_none(),
            "consequent error must not already have a cause"
        );
        Arc::get_mut(inner).unwrap().cause = Some(self);
        err
    }

    #[allow(dead_code)]
    fn root(&self) -> &Error {
        self.chain().last().unwrap()
    }

    fn chain(&self) -> impl Iterator<Item = &Error> {
        let mut err = self;
        core::iter::once(err).chain(core::iter::from_fn(move || {
            err = err.inner.as_ref().and_then(|inner| inner.cause.as_ref())?;
            Some(err)
        }))
    }

    fn kind(&self) -> &ErrorKind {
        self.inner
            .as_ref()
            .map(|inner| &inner.kind)
            .unwrap_or(&ErrorKind::Unknown)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            ErrorKind::Driver(err) => Some(err),
            ErrorKind::ConnectionPool(err) => Some(err),
            ErrorKind::Anyhow(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut it = self.chain().peekable();
        while let Some(err) = it.next() {
            core::fmt::Display::fmt(err.kind(), f)?;
            if it.peek().is_some() {
                f.write_str(": ")?;
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if !f.alternate() {
            core::fmt::Display::fmt(self, f)
        } else {
            let Some(ref inner) = self.inner else {
                return f.debug_struct("Error").field("kind", &"None").finish();
            };
            f.debug_struct("Error")
                .field("kind", &inner.kind)
                .field("cause", &inner.cause)
                .finish()
        }
    }
}

#[derive(Debug)]
enum ErrorKind {
    Anyhow(anyhow::Error),
    Adhoc(AdhocError),
    Driver(DriverError),
    ConnectionPool(ConnectionPoolError),
    TypeConversion(TypeConversionError),
    RecordNotFound(RecordNotFoundError),
    TooManyRecords(TooManyRecordsError),
    InvalidResult(InvalidResultError),
    Validation(ValidationError),
    ConditionFailed(ConditionFailedError),
    Unknown,
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::ErrorKind::*;

        match self {
            Anyhow(err) => core::fmt::Display::fmt(err, f),
            Adhoc(err) => core::fmt::Display::fmt(err, f),
            Driver(err) => core::fmt::Display::fmt(err, f),
            ConnectionPool(err) => core::fmt::Display::fmt(err, f),
            TypeConversion(err) => core::fmt::Display::fmt(err, f),
            RecordNotFound(err) => core::fmt::Display::fmt(err, f),
            TooManyRecords(err) => core::fmt::Display::fmt(err, f),
            InvalidResult(err) => core::fmt::Display::fmt(err, f),
            Validation(err) => core::fmt::Display::fmt(err, f),
            ConditionFailed(err) => core::fmt::Display::fmt(err, f),
            Unknown => f.write_str("unknown toasty error"),
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Some(Arc::new(ErrorInner { kind, cause: None })),
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Error {
        Error::from(ErrorKind::Anyhow(err))
    }
}
impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Error {
        Error::from(anyhow::Error::from(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::from(anyhow::Error::from(err))
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Error {
        Error::from(anyhow::Error::from(err))
    }
}

#[cfg(feature = "jiff")]
impl From<jiff::Error> for Error {
    fn from(err: jiff::Error) -> Error {
        Error::from(anyhow::Error::from(err))
    }
}

/// Trait for types that can be converted into an Error.
pub trait IntoError {
    /// Converts this type into an Error.
    fn into_error(self) -> Error;
}

impl IntoError for Error {
    #[inline(always)]
    fn into_error(self) -> Error {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_size() {
        // Ensure Error stays at one word (size of pointer/Arc)
        let expected_size = core::mem::size_of::<usize>();
        assert_eq!(expected_size, core::mem::size_of::<Error>());
    }

    #[test]
    fn error_from_args() {
        let err = Error::from_args(format_args!("test error: {}", 42));
        assert_eq!(err.to_string(), "test error: 42");
    }

    #[test]
    fn error_chain_display() {
        let root = Error::from_args(format_args!("root cause"));
        let mid = Error::from_args(format_args!("middle context"));
        let top = Error::from_args(format_args!("top context"));

        let chained = root.context(mid).context(top);
        assert_eq!(
            chained.to_string(),
            "top context: middle context: root cause"
        );
    }

    #[test]
    fn anyhow_bridge() {
        // anyhow::Error converts to our Error
        let anyhow_err = anyhow::anyhow!("something failed");
        let our_err: Error = anyhow_err.into();
        assert_eq!(our_err.to_string(), "something failed");
    }

    #[test]
    fn std_error_bridge() {
        // std::io::Error converts via anyhow bridge
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let our_err: Error = io_err.into();
        assert!(our_err.to_string().contains("file not found"));
    }

    #[test]
    fn type_conversion_error() {
        let value = crate::stmt::Value::I64(42);
        let err = Error::type_conversion(value, "String");
        assert_eq!(err.to_string(), "cannot convert I64 to String");
    }

    #[test]
    fn type_conversion_error_range() {
        // Simulates usize conversion failure due to range
        let value = crate::stmt::Value::U64(u64::MAX);
        let err = Error::type_conversion(value, "usize");
        assert_eq!(err.to_string(), "cannot convert U64 to usize");
    }

    #[test]
    fn record_not_found_with_immediate_context() {
        let err = Error::record_not_found("table=users key={id: 123}");
        assert_eq!(
            err.to_string(),
            "record not found: table=users key={id: 123}"
        );
    }

    #[test]
    fn record_not_found_with_context_chain() {
        let err = Error::record_not_found("table=users key={id: 123}")
            .context(err!("update query failed"))
            .context(err!("User.update() operation"));

        assert_eq!(
            err.to_string(),
            "User.update() operation: update query failed: record not found: table=users key={id: 123}"
        );
    }

    #[test]
    fn too_many_records_with_context() {
        let err = Error::too_many_records("expected 1 record, found multiple");
        assert_eq!(
            err.to_string(),
            "too many records: expected 1 record, found multiple"
        );
    }

    #[test]
    fn invalid_result_error() {
        let err = Error::invalid_result("expected Stream, got Count");
        assert_eq!(
            err.to_string(),
            "invalid result: expected Stream, got Count"
        );
    }

    #[test]
    fn validation_length_too_short() {
        let err = Error::validation_length(3, Some(5), Some(10));
        assert_eq!(err.to_string(), "value length 3 is too short (minimum: 5)");
    }

    #[test]
    fn validation_length_too_long() {
        let err = Error::validation_length(15, Some(5), Some(10));
        assert_eq!(err.to_string(), "value length 15 is too long (maximum: 10)");
    }

    #[test]
    fn validation_length_exact_mismatch() {
        let err = Error::validation_length(3, Some(5), Some(5));
        assert_eq!(
            err.to_string(),
            "value length 3 does not match required length 5"
        );
    }

    #[test]
    fn validation_length_min_only() {
        let err = Error::validation_length(3, Some(5), None);
        assert_eq!(err.to_string(), "value length 3 is too short (minimum: 5)");
    }

    #[test]
    fn validation_length_max_only() {
        let err = Error::validation_length(15, None, Some(10));
        assert_eq!(err.to_string(), "value length 15 is too long (maximum: 10)");
    }

    #[test]
    fn condition_failed_with_context() {
        let err = Error::condition_failed("optimistic lock version mismatch");
        assert_eq!(
            err.to_string(),
            "condition failed: optimistic lock version mismatch"
        );
    }

    #[test]
    fn condition_failed_with_format() {
        let expected = 1;
        let actual = 0;
        let err = Error::condition_failed(format!(
            "expected {} row affected, got {}",
            expected, actual
        ));
        assert_eq!(
            err.to_string(),
            "condition failed: expected 1 row affected, got 0"
        );
    }
}
