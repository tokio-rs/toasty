// `#[auto(uuid)]` on an integer field is a strategy/type mismatch — `i64`
// is `AutoCompatible<tag::Increment>`, not `tag::Uuid`. Caught at compile
// time so the user sees the wrong choice instead of a runtime panic.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto(uuid(v7))]
    id: i64,
    title: String,
}

fn main() {}
