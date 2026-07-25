pub(crate) fn unsupported(operation: &str) -> toasty_core::Error {
    toasty_core::Error::unsupported_feature(format!(
        "D1 does not support the {operation} operation"
    ))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn worker(operation: &str, error: worker::Error) -> toasty_core::Error {
    let error = std::io::Error::other(error.to_string());
    toasty_core::Error::driver_operation_failed(error).context(toasty_core::Error::from_args(
        format_args!("D1 {operation} failed"),
    ))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn result(operation: &str, message: String) -> toasty_core::Error {
    let error = std::io::Error::other(message);
    toasty_core::Error::driver_operation_failed(error).context(toasty_core::Error::from_args(
        format_args!("D1 {operation} failed"),
    ))
}
