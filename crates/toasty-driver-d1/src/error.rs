pub(crate) fn unsupported(operation: &str) -> toasty_core::Error {
    toasty_core::Error::unsupported_feature(format!(
        "D1 does not support the {operation} operation"
    ))
}
