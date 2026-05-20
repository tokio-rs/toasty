// The nested-`Option` rejection also covers a deferred field whose inner type
// is `Option<Option<T>>`: `Deferred<T>: Field` requires `T: Field`, and
// `Option<Option<T>>` does not satisfy it because the inner `Option<T>` is not
// `NotNullable`.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto]
    id: u64,

    #[deferred]
    flag: toasty::Deferred<Option<Option<bool>>>,
}

fn main() {}
