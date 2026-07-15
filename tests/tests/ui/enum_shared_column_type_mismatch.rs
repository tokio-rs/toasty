// Two variants declare the same `#[shared(value)]` field but with different
// types. A shared column has a single storage type, so the `SameColumnType`
// obligation must reject the mismatch.

#[derive(Debug, toasty::Embed)]
enum Value {
    #[column(variant = 1)]
    Text {
        #[shared(value)]
        text: String,
    },
    #[column(variant = 2)]
    Number {
        #[shared(value)]
        number: i64,
    },
}

fn main() {}
