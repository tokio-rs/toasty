// `String` is shadowed by a local type alias — the macro sees the identifier
// `String` but the actual Rust type is `u32`. A macro that parsed types
// statically would miss this. The `CompatibleWith` check goes through trait
// resolution, so it correctly rejects the mismatch (`#[column(type = text)]`
// is incompatible with the resolved `u32`).

#[allow(non_camel_case_types)]
type String = u32;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: i64,

    #[column(type = text)]
    name: String,
}

fn main() {}
