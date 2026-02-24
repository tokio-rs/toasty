/// Redact the password portion of a database URL for safe display.
///
/// If the URL can be parsed and contains a password, replaces it with `***`.
/// If parsing fails (e.g. `sqlite::memory:`), returns the original string unchanged.
pub(crate) fn redact_url_password(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        }
        Err(_) => url.to_string(),
    }
}
