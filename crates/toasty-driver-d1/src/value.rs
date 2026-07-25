pub(crate) const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_991;
pub(crate) const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub(crate) fn validate_i64(value: i64) -> toasty_core::Result<()> {
    if (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value) {
        Ok(())
    } else {
        Err(toasty_core::Error::validation_failed(format!(
            "D1 integer is outside JavaScript's safe range ({MIN_SAFE_INTEGER}..={MAX_SAFE_INTEGER})"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_javascript_safe_integer_bounds() {
        assert!(validate_i64(MIN_SAFE_INTEGER).is_ok());
        assert!(validate_i64(MAX_SAFE_INTEGER).is_ok());
        assert!(validate_i64(MIN_SAFE_INTEGER - 1).is_err());
        assert!(validate_i64(MAX_SAFE_INTEGER + 1).is_err());
    }
}
