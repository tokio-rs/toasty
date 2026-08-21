use toasty_core::stmt::Type;

#[test]
fn equality_preserving_casts_are_allowlisted() {
    assert!(Type::Bytes.cast_preserves_equality(&Type::Uuid));
    assert!(Type::String.cast_preserves_equality(&Type::Uuid));
    assert!(Type::U8.cast_preserves_equality(&Type::I64));
    assert!(Type::list(Type::U8).cast_preserves_equality(&Type::list(Type::I64)));
}

#[test]
fn lossy_and_unreviewed_casts_are_rejected() {
    assert!(!Type::F64.cast_preserves_equality(&Type::F32));
    assert!(!Type::I8.cast_preserves_equality(&Type::Bool));

    #[cfg(feature = "rust_decimal")]
    assert!(!Type::String.cast_preserves_equality(&Type::Decimal));

    #[cfg(feature = "bigdecimal")]
    assert!(!Type::String.cast_preserves_equality(&Type::BigDecimal));
}
