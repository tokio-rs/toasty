// A model field of type `Option<Option<T>>` cannot be represented: a column
// has one NULL value, so both `Option` layers collapse onto it and
// `Some(None)` would read back as `None` (silent data loss). Toasty rejects
// nested `Option` at compile time with the `Present` bound on
// `Field for Option<T>`.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto]
    id: u64,

    flag: Option<Option<bool>>,
}

fn main() {}
