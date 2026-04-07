/// Metadata about a model's fields for compile-time validation of `create!` invocations.
pub struct CreateMeta {
    /// The fields available for creation (excludes auto, default, update, FK source,
    /// relation, and serialized fields).
    pub fields: &'static [CreateField],
    /// The model name, used in error messages.
    pub model_name: &'static str,
}

/// A single field's metadata for create validation.
pub struct CreateField {
    /// The field name as it appears in the `create!` macro.
    pub name: &'static str,
    /// Whether this field must be provided. Computed from `!<T as Field>::NULLABLE`.
    pub required: bool,
}

/// Trait that carries [`CreateMeta`] for compile-time validation.
///
/// Implemented on fields structs and relation scope types so that
/// the `create!` macro can validate field sets through monomorphization.
#[doc(hidden)]
pub trait ValidateCreate {
    const CREATE_META: &'static CreateMeta;
}

/// Panics at compile time if any required field from `meta` is missing from `provided`.
///
/// Uses byte-level string comparison because `const fn` cannot call trait methods
/// like `PartialEq`.
///
/// This is used in monomorphization-based validation (scoped and nested creates)
/// where the concrete model type is not known at macro expansion time.
/// For typed creates, the `create!` macro calls a per-model `const fn` that
/// produces field-specific panic messages instead.
pub const fn assert_create_fields(meta: &CreateMeta, provided: &[&str]) {
    let mut i = 0;
    while i < meta.fields.len() {
        let field = &meta.fields[i];
        if field.required && !const_contains(provided, field.name) {
            panic!(
                "missing required field in create! macro — check model definition for required fields"
            );
        }
        i += 1;
    }
}

/// Byte-level string containment check usable in `const fn`.
///
/// Returns `true` if any element of `haystack` equals `needle`.
pub const fn const_contains(haystack: &[&str], needle: &str) -> bool {
    let mut i = 0;
    while i < haystack.len() {
        if const_str_eq(haystack[i], needle) {
            return true;
        }
        i += 1;
    }
    false
}

const fn const_str_eq(a: &str, b: &str) -> bool {
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
