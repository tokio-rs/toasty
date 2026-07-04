// Two variants map a field to the same `#[column("value")]` but with different
// types. A shared column has a single storage type, so the `SameColumnType`
// obligation must reject the mismatch.

#[derive(Debug, toasty::Embed)]
enum Value {
    #[column(variant = 1)]
    Text {
        #[column("value")]
        text: String,
    },
    #[column(variant = 2)]
    Number {
        #[column("value")]
        number: i64,
    },
}

fn main() {}
