use crate::Error;

#[derive(Debug)]
pub(super) struct InvalidConnectionUrl {
    pub(super) message: Box<str>,
}

impl Error {
    pub fn invalid_connection_url(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidConnectionUrl(
            InvalidConnectionUrl {
                message: message.into().into(),
            },
        ))
    }

    pub fn is_invalid_connection_url(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidConnectionUrl(_))
    }
}

impl std::fmt::Display for InvalidConnectionUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid connection URL: {}", self.message)
    }
}
