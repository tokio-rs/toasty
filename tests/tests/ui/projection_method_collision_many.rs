// `insert` is a method only on `Many<Direct>` (not on the model's query
// struct), so the projection-method skip applies only to the `Many` wrapper.
// `#![deny(deprecated)]` upgrades the warning into a hard error so trybuild
// captures it.

#![deny(deprecated)]

#[derive(Debug, toasty::Model)]
struct Widget {
    #[key]
    #[auto]
    id: i64,
    insert: String,
}

fn main() {}
