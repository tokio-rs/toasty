use crate::stmt::Value;

/// Error when a value cannot be converted to the expected type.
#[derive(Debug)]
pub(super) struct TypeConversionError {
    pub(super) value: Value,
    pub(super) to_type: &'static str,
}

impl std::error::Error for TypeConversionError {}

impl core::fmt::Display for TypeConversionError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "cannot convert {:?} to {}",
            self.value.infer_ty(),
            self.to_type
        )
    }
}
