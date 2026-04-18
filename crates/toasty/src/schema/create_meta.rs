//! Compile-time metadata for validating `create!` macro invocations.
//!
//! The [`create!`](crate::create) macro uses [`CreateMeta`] to check at
//! compile time that every required field of the target model has been
//! specified. See the design doc
//! `docs/dev/src/design/static-assertions-create-macro.md` for background.

/// A compile-time description of the fields a model exposes through `create!`.
///
/// `CreateMeta` is flat — it does not reference other models' metadata.
/// Cross-model resolution for nested and scoped `create!` invocations happens
/// at monomorphization time via the [`ValidateCreate`] trait.
pub struct CreateMeta {
    /// The fields exposed through `create!` for this model.
    pub fields: &'static [CreateField],
    /// The name of the model (used in error messages).
    pub model_name: &'static str,
}

/// A single field entry in a [`CreateMeta`].
pub struct CreateField {
    /// The field's Rust identifier.
    pub name: &'static str,
    /// Whether the field must be specified in a `create!` invocation.
    pub required: bool,
    /// Pre-formatted panic message used when this required field is missing.
    ///
    /// The message is assembled at derive time (where `format!` is
    /// available) so the `const fn` checker can pass it straight to
    /// `panic!` — `const fn` on stable does not allow formatted panics.
    pub missing_message: &'static str,
}

/// Trait carried by generated types that can resolve to a target model's
/// [`CreateMeta`].
///
/// The `create!` macro uses this trait to perform per-level validation at
/// monomorphization time. The trait is implemented on the generated
/// `FooFields`/`FooListFields` structs and on the relation scope types
/// (`Many`, `One`, `OptionOne`) so that expressions like
/// `User::fields().todos()` or `user.todos()` carry the target model's
/// metadata.
///
/// This is an implementation detail of the `create!` macro and is not part
/// of the public API.
#[doc(hidden)]
pub trait ValidateCreate {
    /// The `CreateMeta` for the model this type targets.
    const CREATE_META: &'static CreateMeta;
}

/// Assert that `provided` contains every required field listed in `meta`.
///
/// This is a `const fn` helper called from the `create!` macro expansion.
/// If a required field is missing, it panics with the field's
/// pre-formatted `missing_message`, which names the missing field and its
/// model. The Rust compiler surfaces the panic as an `error[E0080]` at the
/// `create!` call site.
///
/// The implementation uses byte-level string comparison because `const fn`
/// cannot call trait methods like `PartialEq`, and cannot call the
/// formatting machinery used by `panic!("{}", ...)`.
#[track_caller]
pub const fn assert_create_fields(meta: &CreateMeta, provided: &[&str]) {
    let mut i = 0;
    while i < meta.fields.len() {
        let field = &meta.fields[i];
        if field.required && !contains_str(provided, field.name) {
            panic!("{}", field.missing_message);
        }
        i += 1;
    }
}

/// Returns `true` if `needle` is present in `haystack`, using byte-level
/// comparison.
const fn contains_str(haystack: &[&str], needle: &str) -> bool {
    let mut i = 0;
    while i < haystack.len() {
        if str_eq(haystack[i], needle) {
            return true;
        }
        i += 1;
    }
    false
}

/// Byte-level equality check usable in `const fn` context.
const fn str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
