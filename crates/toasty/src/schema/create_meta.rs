/// Metadata describing the fields a model exposes on creation.
///
/// This struct is used at compile time to verify that all required fields
/// are provided in a `create!` invocation. Each model's `#[derive(Model)]`
/// expansion produces a `const CREATE_META: CreateMeta` that lists the
/// primitive fields available for creation, any nested relation creates,
/// and the `BelongsTo` relations (so nested creates can skip FK fields
/// provided by the parent context).
pub struct CreateMeta {
    /// Primitive fields available for creation (excludes auto, default,
    /// update, relation, and FK source fields).
    pub fields: &'static [CreateField],

    /// Nested `HasMany` / `HasOne` relations that can be created inline.
    pub nested: &'static [CreateNested],

    /// `BelongsTo` relations declared on this model, used by the parent
    /// side to discover which FK fields the relationship provides.
    pub belongs_to: &'static [CreateBelongsTo],

    /// The model name, used in error messages.
    pub model_name: &'static str,
}

/// A single primitive field in a `CreateMeta`.
pub struct CreateField {
    /// The field name as it appears in the `create!` macro.
    pub name: &'static str,

    /// Whether this field is required. Computed as `!<T as Field>::NULLABLE`.
    pub required: bool,
}

/// A `BelongsTo` relation on the model. Lists the FK source fields so that
/// a parent nested create can skip them.
pub struct CreateBelongsTo {
    /// The relation field name (e.g., `"user"`).
    pub name: &'static str,

    /// The FK source field names (e.g., `&["user_id"]`).
    pub fk_fields: &'static [&'static str],
}

/// A nested relation (`HasMany` or `HasOne`) that can be created inline.
pub struct CreateNested {
    /// The relation field name (e.g., `"todos"`).
    pub name: &'static str,

    /// The child model's `CreateMeta`.
    pub meta: &'static CreateMeta,

    /// The name of the `BelongsTo` field on the child that this relation
    /// satisfies (e.g., `"user"`).
    pub pair: &'static str,
}

// === const fn helpers ===

/// Assert that all required fields are present in `provided`.
///
/// This function is called inside a `const _: () = { ... }` block so that
/// a missing required field causes a compile-time error.
pub const fn assert_create_fields(meta: &CreateMeta, provided: &[&str]) {
    let mut i = 0;
    while i < meta.fields.len() {
        let field = &meta.fields[i];
        if field.required && !str_slice_contains(provided, field.name) {
            missing_field_panic(meta.model_name, field.name);
        }
        i += 1;
    }
}

/// Assert that all required fields are present for a nested create.
///
/// Looks up `field_name` in `meta.nested`, retrieves the child's
/// `CreateMeta`, finds the matching `BelongsTo` entry via the `pair`
/// name, and skips those FK fields when checking.
pub const fn assert_nested_create_fields(meta: &CreateMeta, field_name: &str, provided: &[&str]) {
    // Look up the nested entry. If not found, the field might be a
    // BelongsTo inline create or a self-referential relation — skip.
    let Some(nested) = try_find_nested(meta, field_name) else {
        return;
    };
    let child_meta = nested.meta;
    let pair = nested.pair;

    // Collect FK fields to skip from the child's belongs_to entry
    let belongs_to = find_belongs_to(child_meta, pair);
    let skip_fields = belongs_to.fk_fields;

    let mut i = 0;
    while i < child_meta.fields.len() {
        let field = &child_meta.fields[i];
        if field.required
            && !str_slice_contains(provided, field.name)
            && !str_slice_contains(skip_fields, field.name)
        {
            missing_field_panic(child_meta.model_name, field.name);
        }
        i += 1;
    }
}

// === internal helpers ===

const fn try_find_nested<'a>(meta: &'a CreateMeta, name: &str) -> Option<&'a CreateNested> {
    let mut i = 0;
    while i < meta.nested.len() {
        if const_str_eq(meta.nested[i].name, name) {
            return Some(&meta.nested[i]);
        }
        i += 1;
    }
    None
}

const fn find_belongs_to<'a>(meta: &'a CreateMeta, name: &str) -> &'a CreateBelongsTo {
    let mut i = 0;
    while i < meta.belongs_to.len() {
        if const_str_eq(meta.belongs_to[i].name, name) {
            return &meta.belongs_to[i];
        }
        i += 1;
    }
    panic!("belongs_to field not found in CreateMeta")
}

const fn str_slice_contains(slice: &[&str], needle: &str) -> bool {
    let mut i = 0;
    while i < slice.len() {
        if const_str_eq(slice[i], needle) {
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

/// Panics with a message indicating which required field is missing.
///
/// In const context, `panic!` only accepts `&str` literals or `&str` values.
/// We cannot use `format!`, so we include the model name and field name
/// separately. The compiler error output shows the full panic message
/// including both names.
#[track_caller]
const fn missing_field_panic(model_name: &str, field_name: &str) {
    // Build the message in a fixed buffer using only const-safe operations.
    const PREFIX: &[u8] = b"missing required field `";
    const MID: &[u8] = b"` in create! for `";
    const SUFFIX: &[u8] = b"`";

    let field_bytes = field_name.as_bytes();
    let model_bytes = model_name.as_bytes();
    let total_len = PREFIX.len() + field_bytes.len() + MID.len() + model_bytes.len() + SUFFIX.len();

    assert!(
        total_len <= 256,
        "field/model name too long for error buffer"
    );

    let mut buf = [0u8; 256];
    let mut pos = 0;

    pos = copy_bytes(&mut buf, pos, PREFIX);
    pos = copy_bytes(&mut buf, pos, field_bytes);
    pos = copy_bytes(&mut buf, pos, MID);
    pos = copy_bytes(&mut buf, pos, model_bytes);
    pos = copy_bytes(&mut buf, pos, SUFFIX);

    let msg = const_buf_to_str(&buf, pos);
    panic!("{}", msg);
}

const fn copy_bytes(buf: &mut [u8; 256], start: usize, src: &[u8]) -> usize {
    let mut i = 0;
    while i < src.len() {
        buf[start + i] = src[i];
        i += 1;
    }
    start + src.len()
}

/// Convert the first `len` bytes of `buf` to a `&str` in const context.
///
/// All inputs are guaranteed to be valid ASCII (Rust identifiers + ASCII
/// literal prefixes), so UTF-8 validity is ensured by construction.
const fn const_buf_to_str(buf: &[u8; 256], len: usize) -> &str {
    // Validate UTF-8 byte by byte (all our inputs are ASCII, so this
    // always succeeds).
    let mut i = 0;
    while i < len {
        assert!(buf[i] < 128, "non-ASCII byte in error message buffer");
        i += 1;
    }

    // Use str::from_utf8 via match — available in const since Rust 1.63.
    // We need a slice of the right length. Since we can't do &buf[..len]
    // in older const contexts, we use core::str::from_utf8 on the full
    // buffer and then rely on the panic approach below.

    // Actually, since Rust 1.80 we can use &buf[..len] in const fn.
    // For broader compatibility, use a helper that builds a &[u8] via
    // core::slice::from_raw_parts equivalent.

    // The simplest const-safe approach: validate and transmute.
    // Since #![forbid(unsafe_code)] is active, we use a different trick:
    // build the string at compile time using only safe const operations.

    // We assert all bytes are ASCII above, so we can match on from_utf8.
    match core::str::from_utf8(buf.split_at(len).0) {
        Ok(s) => s,
        Err(_) => panic!("invalid UTF-8 in error message buffer"),
    }
}
