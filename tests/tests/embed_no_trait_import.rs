//! Test that `#[derive(Embed)]` works without importing toasty traits.
//!
//! Regression test: the generated code previously called `Self::id()` inside
//! the `Primitive` impl without qualifying the `Register` trait, causing
//! `E0599: no variant or associated item named 'id'` when the user had not
//! imported `toasty::Register` (or `toasty::Embed`) in scope.

// Intentionally no `use toasty::*` or `use toasty::{Embed, Register}` here.

#[derive(Debug, toasty::Embed)]
pub enum AuthTokenKind {
    #[column(variant = 0)]
    EmailVerification,
    #[column(variant = 1)]
    PasswordReset,
}

#[derive(Debug, toasty::Embed)]
pub struct Address {
    street: String,
    city: String,
}

/// If this compiles, the fix is working: generated code fully qualifies trait methods.
#[test]
fn embed_compiles_without_trait_imports() {
    let _ = AuthTokenKind::EmailVerification;
    let _ = Address {
        street: "123 Main".into(),
        city: "Springfield".into(),
    };
}
