// Two fields in the same variant map to the same `#[column("x")]`. A column can
// be shared only across different variants (at most one is active per row), so a
// same-variant collision is an invalid mapping and must be rejected.

#[derive(Debug, toasty::Embed)]
enum Value {
    #[column(variant = 1)]
    A {
        #[column("x")]
        left: String,
        #[column("x")]
        right: String,
    },
}

fn main() {}
