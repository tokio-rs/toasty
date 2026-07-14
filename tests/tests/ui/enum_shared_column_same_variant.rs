// Two fields in the same variant declare `#[shared(x)]`. A column can be
// shared only across different variants (at most one is active per row), so a
// same-variant collision is an invalid mapping and must be rejected.

#[derive(Debug, toasty::Embed)]
enum Value {
    #[column(variant = 1)]
    A {
        #[shared(x)]
        left: String,
        #[shared(x)]
        right: String,
    },
}

fn main() {}
