mod record_not_found;
mod type_conversion;

use self::record_not_found::RecordNotFoundError;
use self::type_conversion::TypeConversionError;
use std::sync::Arc;

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
    /// Creates an error from a format string.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// let err = Error::from_args(format_args!("value {} is invalid", "foo"));
    /// ```
    pub fn from_args<'a>(message: core::fmt::Arguments<'a>) -> Error {
        Error::from(ErrorKind::Adhoc(AdhocError::from_args(message)))
    }

    /// Creates an error from a driver error.
    ///
    /// This is the preferred way to convert driver-specific errors (rusqlite, tokio-postgres,
    /// mysql_async, AWS SDK errors, etc.) into toasty errors.
    pub fn driver(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(ErrorKind::Driver(Box::new(err)))
    }

    /// Creates an error from a connection pool error.
    ///
    /// This is used for errors that occur when managing the connection pool (e.g., deadpool errors).
    pub fn connection_pool(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(ErrorKind::ConnectionPool(Box::new(err)))
    }

    /// Creates a type conversion error.
    ///
    /// This is used when a value cannot be converted to the expected type.
    pub fn type_conversion(value: crate::stmt::Value, to_type: &'static str) -> Error {
        Error::from(ErrorKind::TypeConversion(
            type_conversion::TypeConversionError { value, to_type },
        ))
    }

    /// Creates a record not found error.
    ///
    /// This is the root cause error when a record lookup (by query or key) returns no results.
    ///
    /// The context parameter provides immediate context about what was not found.
    /// Additional context can be added at each layer via `.context()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// // With context describing what wasn't found (string literal)
    /// let err = Error::record_not_found("table=users key={id: 123}");
    /// assert_eq!(err.to_string(), "record not found: table=users key={id: 123}");
    ///
    /// // With context from format! or String
    /// let table = "users";
    /// let key = 123;
    /// let err = Error::record_not_found(format!("table={} key={}", table, key));
    /// assert_eq!(err.to_string(), "record not found: table=users key=123");
    /// ```
    pub fn record_not_found(context: impl Into<String>) -> Error {
        Error::from(ErrorKind::RecordNotFound(
            record_not_found::RecordNotFoundError::new(Some(context.into().into())),
        ))
    }

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
            ErrorKind::Driver(err) => Some(err.as_ref()),
            ErrorKind::ConnectionPool(err) => Some(err.as_ref()),
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
    Driver(Box<dyn std::error::Error + Send + Sync>),
    ConnectionPool(Box<dyn std::error::Error + Send + Sync>),
    TypeConversion(TypeConversionError),
    RecordNotFound(RecordNotFoundError),
    Unknown,
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::ErrorKind::*;

        match self {
            Anyhow(err) => core::fmt::Display::fmt(err, f),
            Adhoc(err) => core::fmt::Display::fmt(err, f),
            Driver(err) => {
                // Display the error and walk its source chain
                core::fmt::Display::fmt(err, f)?;
                let mut source = err.source();
                while let Some(err) = source {
                    write!(f, ": {}", err)?;
                    source = err.source();
                }
                Ok(())
            }
            ConnectionPool(err) => {
                // Display the error and walk its source chain
                core::fmt::Display::fmt(err, f)?;
                let mut source = err.source();
                while let Some(err) = source {
                    write!(f, ": {}", err)?;
                    source = err.source();
                }
                Ok(())
            }
            TypeConversion(err) => core::fmt::Display::fmt(err, f),
            RecordNotFound(err) => core::fmt::Display::fmt(err, f),
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

struct AdhocError {
    message: Box<str>,
}

impl AdhocError {
    fn from_args<'a>(message: core::fmt::Arguments<'a>) -> AdhocError {
        use std::string::ToString;

        let message = message.to_string().into_boxed_str();
        AdhocError { message }
    }
}

impl std::error::Error for AdhocError {}

impl core::fmt::Display for AdhocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.message, f)
    }
}

impl core::fmt::Debug for AdhocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.message, f)
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
}
