// A model field whose name matches a method on `{Model}Query` (here, `count`)
// triggers a `#[deprecated]` warning that points the user at `.select(...)`.
// `#![deny(deprecated)]` upgrades the warning so trybuild captures it.

#![deny(deprecated)]

#[derive(Debug, toasty::Model)]
struct Widget {
    #[key]
    #[auto]
    id: i64,
    count: i64,
}

fn main() {}
