// `#[auto(uuid)]` requires a UUID-compatible field type. `bool` is rejected
// at compile time by the `AutoCompatible<tag::Uuid>` obligation rather than
// silently compiling and panicking at insert time.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto(uuid(v7))]
    id: bool,
    title: String,
}

fn main() {}
