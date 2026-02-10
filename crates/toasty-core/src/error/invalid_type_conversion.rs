use super::Error;
use crate::stmt::Value;

/// Error when a value cannot be converted to the expected type.
#[derive(Debug)]
pub(super) struct InvalidTypeConversion {
    pub(super) value: Value,
    pub(super) to_type: &'static str,
}

impl std::error::Error for InvalidTypeConversion {}

impl core::fmt::Display for InvalidTypeConversion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "cannot convert {:?} to {}",
            self.value.infer_ty(),
            self.to_type
        )
    }
}

impl Error {
    /// Creates a type conversion error.
    ///
    /// This is used when a value cannot be converted to the expected type.
    pub fn type_conversion(value: crate::stmt::Value, to_type: &'static str) -> Error {
        Error::from(super::ErrorKind::InvalidTypeConversion(
            InvalidTypeConversion { value, to_type },
        ))
    }

    /// Returns `true` if this error is a type conversion error.
    pub fn is_type_conversion(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidTypeConversion(_))
    }
}
