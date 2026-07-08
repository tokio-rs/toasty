// Two variants map a field to the same `#[column("value")]` without declaring
// a shared field. Sharing is always explicit; a bare column-name collision is a
// duplicate column, not an implicit merge.

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
        number: String,
    },
}

fn main() {}
