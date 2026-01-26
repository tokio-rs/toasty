/// Error when a value fails validation constraints.
#[derive(Debug)]
pub(super) struct ValidationError {
    pub(super) kind: ValidationErrorKind,
}

#[derive(Debug)]
pub(super) enum ValidationErrorKind {
    /// String length constraint violation
    Length {
        value_len: usize,
        min: Option<usize>,
        max: Option<usize>,
    },
}

impl std::error::Error for ValidationError {}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match &self.kind {
            ValidationErrorKind::Length {
                value_len,
                min,
                max,
            } => {
                // If min and max are the same, show exact length requirement
                if min == max && min.is_some() {
                    let expected = min.unwrap();
                    return write!(
                        f,
                        "value length {} does not match required length {}",
                        value_len, expected
                    );
                }

                // Check which constraint was violated
                let too_short = min.map_or(false, |m| *value_len < m);
                let too_long = max.map_or(false, |m| *value_len > m);

                if too_short {
                    write!(
                        f,
                        "value length {} is too short (minimum: {})",
                        value_len,
                        min.unwrap()
                    )
                } else if too_long {
                    write!(
                        f,
                        "value length {} is too long (maximum: {})",
                        value_len,
                        max.unwrap()
                    )
                } else {
                    f.write_str("length constraint violation")
                }
            }
        }
    }
}
