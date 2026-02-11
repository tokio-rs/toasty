mod adhoc;
mod condition_failed;
mod connection_pool;
mod driver_operation_failed;
mod expression_evaluation_failed;
mod invalid_connection_url;
mod invalid_driver_configuration;
mod invalid_record_count;
mod invalid_result;
mod invalid_schema;
mod invalid_statement;
mod invalid_type_conversion;
mod record_not_found;
mod unsupported_feature;
mod validation;

use adhoc::Adhoc;
use condition_failed::ConditionFailed;
use connection_pool::ConnectionPool;
use driver_operation_failed::DriverOperationFailed;
use expression_evaluation_failed::ExpressionEvaluationFailed;
use invalid_connection_url::InvalidConnectionUrl;
use invalid_driver_configuration::InvalidDriverConfiguration;
use invalid_record_count::InvalidRecordCount;
use invalid_result::InvalidResult;
use invalid_schema::InvalidSchema;
use invalid_statement::InvalidStatement;
use invalid_type_conversion::InvalidTypeConversion;
use record_not_found::RecordNotFound;
use std::sync::Arc;
use unsupported_feature::UnsupportedFeature;
use validation::ValidationFailed;

/// An error that can occur in Toasty.
#[derive(Clone)]
pub struct Error {
    inner: Arc<ErrorInner>,
}

/// Trait for types that can be converted into an Error.
pub trait IntoError {
    /// Converts this type into an Error.
    fn into_error(self) -> Error;
}

#[derive(Debug)]
struct ErrorInner {
    kind: ErrorKind,
    cause: Option<Error>,
}

#[derive(Debug)]
enum ErrorKind {
    Adhoc(Adhoc),
    DriverOperationFailed(DriverOperationFailed),
    ConnectionPool(ConnectionPool),
    ExpressionEvaluationFailed(ExpressionEvaluationFailed),
    InvalidConnectionUrl(InvalidConnectionUrl),
    InvalidDriverConfiguration(InvalidDriverConfiguration),
    InvalidTypeConversion(InvalidTypeConversion),
    InvalidRecordCount(InvalidRecordCount),
    RecordNotFound(RecordNotFound),
    InvalidResult(InvalidResult),
    InvalidSchema(InvalidSchema),
    InvalidStatement(InvalidStatement),
    UnsupportedFeature(UnsupportedFeature),
    ValidationFailed(ValidationFailed),
    ConditionFailed(ConditionFailed),
}

impl Error {
    /// Adds context to this error.
    ///
    /// Context is displayed in reverse order: the most recently added context is shown first,
    /// followed by earlier context, ending with the root cause.
    pub fn context(self, consequent: impl IntoError) -> Error {
        self.context_impl(consequent.into_error())
    }

    fn context_impl(self, consequent: Error) -> Error {
        let mut err = consequent;
        let inner = Arc::get_mut(&mut err.inner).unwrap();
        assert!(
            inner.cause.is_none(),
            "consequent error must not already have a cause"
        );
        inner.cause = Some(self);
        err
    }

    fn chain(&self) -> impl Iterator<Item = &Error> {
        let mut err = self;
        core::iter::once(err).chain(core::iter::from_fn(move || {
            err = err.inner.cause.as_ref()?;
            Some(err)
        }))
    }

    fn kind(&self) -> &ErrorKind {
        &self.inner.kind
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            ErrorKind::DriverOperationFailed(err) => Some(err),
            ErrorKind::ConnectionPool(err) => Some(err),
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
            f.debug_struct("Error")
                .field("kind", &self.inner.kind)
                .field("cause", &self.inner.cause)
                .finish()
        }
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::ErrorKind::*;

        match self {
            Adhoc(err) => core::fmt::Display::fmt(err, f),
            DriverOperationFailed(err) => core::fmt::Display::fmt(err, f),
            ConnectionPool(err) => core::fmt::Display::fmt(err, f),
            ExpressionEvaluationFailed(err) => core::fmt::Display::fmt(err, f),
            InvalidConnectionUrl(err) => core::fmt::Display::fmt(err, f),
            InvalidDriverConfiguration(err) => core::fmt::Display::fmt(err, f),
            InvalidTypeConversion(err) => core::fmt::Display::fmt(err, f),
            InvalidRecordCount(err) => core::fmt::Display::fmt(err, f),
            RecordNotFound(err) => core::fmt::Display::fmt(err, f),
            InvalidResult(err) => core::fmt::Display::fmt(err, f),
            InvalidSchema(err) => core::fmt::Display::fmt(err, f),
            InvalidStatement(err) => core::fmt::Display::fmt(err, f),
            UnsupportedFeature(err) => core::fmt::Display::fmt(err, f),
            ValidationFailed(err) => core::fmt::Display::fmt(err, f),
            ConditionFailed(err) => core::fmt::Display::fmt(err, f),
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Arc::new(ErrorInner { kind, cause: None }),
        }
    }
}

impl IntoError for Error {
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
            .context(Error::from_args(format_args!("update query failed")))
            .context(Error::from_args(format_args!("User.update() operation")));

        assert_eq!(
            err.to_string(),
            "User.update() operation: update query failed: record not found: table=users key={id: 123}"
        );
    }

    #[test]
    fn invalid_record_count_with_context() {
        let err = Error::invalid_record_count("expected 1 record, found multiple");
        assert_eq!(
            err.to_string(),
            "invalid record count: expected 1 record, found multiple"
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

    #[test]
    fn invalid_schema_error() {
        let err = Error::invalid_schema("duplicate index name `idx_users`");
        assert_eq!(
            err.to_string(),
            "invalid schema: duplicate index name `idx_users`"
        );
    }

    #[test]
    fn invalid_schema_with_context() {
        let err = Error::invalid_schema(
            "auto_increment column `id` in table `users` must have a numeric type, found String",
        )
        .context(Error::from_args(format_args!("schema verification failed")));
        assert_eq!(
            err.to_string(),
            "schema verification failed: invalid schema: auto_increment column `id` in table `users` must have a numeric type, found String"
        );
    }

    #[test]
    fn expression_evaluation_failed() {
        let err = Error::expression_evaluation_failed("failed to resolve argument");
        assert_eq!(
            err.to_string(),
            "expression evaluation failed: failed to resolve argument"
        );
    }

    #[test]
    fn expression_evaluation_failed_with_context() {
        let err = Error::expression_evaluation_failed("expected boolean value")
            .context(Error::from_args(format_args!("query execution failed")));
        assert_eq!(
            err.to_string(),
            "query execution failed: expression evaluation failed: expected boolean value"
        );
    }

    #[test]
    fn unsupported_feature() {
        let err = Error::unsupported_feature("VARCHAR type is not supported by this database");
        assert_eq!(
            err.to_string(),
            "unsupported feature: VARCHAR type is not supported by this database"
        );
    }

    #[test]
    fn unsupported_feature_with_context() {
        let err = Error::unsupported_feature("type List is not supported by this database")
            .context(Error::from_args(format_args!("schema creation failed")));
        assert_eq!(
            err.to_string(),
            "schema creation failed: unsupported feature: type List is not supported by this database"
        );
    }

    #[test]
    fn invalid_driver_configuration() {
        let err = Error::invalid_driver_configuration(
            "native_varchar is true but storage_types.varchar is None",
        );
        assert_eq!(
            err.to_string(),
            "invalid driver configuration: native_varchar is true but storage_types.varchar is None"
        );
    }

    #[test]
    fn invalid_driver_configuration_with_context() {
        let err = Error::invalid_driver_configuration("inconsistent capability flags").context(
            Error::from_args(format_args!("driver initialization failed")),
        );
        assert_eq!(
            err.to_string(),
            "driver initialization failed: invalid driver configuration: inconsistent capability flags"
        );
    }

    #[test]
    fn invalid_statement_error() {
        let err = Error::invalid_statement("field `unknown_field` does not exist on model `User`");
        assert_eq!(
            err.to_string(),
            "invalid statement: field `unknown_field` does not exist on model `User`"
        );
    }

    #[test]
    fn invalid_statement_with_context() {
        let err = Error::invalid_statement("cannot update primary key field `id`")
            .context(Error::from_args(format_args!("statement lowering failed")));
        assert_eq!(
            err.to_string(),
            "statement lowering failed: invalid statement: cannot update primary key field `id`"
        );
    }
}
