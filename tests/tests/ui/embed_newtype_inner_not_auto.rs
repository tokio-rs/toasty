// A newtype embed around a non-`Auto` type cannot be used as an `#[auto]`
// field — the blanket impl in `codegen_support` requires the inner type to
// implement `Auto`, and `String` does not.

#[derive(Debug, toasty::Embed)]
struct UserId(String);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: UserId,
    name: String,
}

fn main() {}
