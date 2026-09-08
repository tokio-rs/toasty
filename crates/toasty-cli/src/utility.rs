/// Redact the password portion of a database URL for safe display.
///
/// If the URL can be parsed and contains a password, replaces it with `***`.
/// If parsing fails, returns the original string unchanged.
pub(crate) fn redact_url_password(url: &str) -> String {
    toasty_core::driver::ConnectionUrl::parse(url)
        .map(|url| url.redact_password().into_owned())
        .unwrap_or_else(|_| url.to_string())
}
