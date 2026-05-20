// A model field of type `Option<Option<T>>` has no on-the-wire encoding: both
// `Option` layers collapse onto the column's single NULL channel, so a
// `Some(None)` value would round-trip indistinguishably from `None` (silent
// data loss). Toasty rejects nested `Option` at compile time via the
// `Present` bound on `Field for Option<T>`.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto]
    id: u64,

    flag: Option<Option<bool>>,
}

fn main() {}
