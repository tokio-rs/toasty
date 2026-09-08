// Index attributes belong on the named field that uses a newtype. The inner
// tuple field is transparent and does not have an application-level name.

#[derive(Debug, toasty::Embed)]
struct Indexed(#[index] String);

#[derive(Debug, toasty::Embed)]
struct Unique(#[unique] String);

fn main() {}
