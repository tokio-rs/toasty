// `#[auto(increment)]` on a UUID field is a strategy/type mismatch — `Uuid`
// is `AutoCompatible<tag::Uuid>`, not `tag::Increment`.

#[derive(Debug, toasty::Model)]
struct Doc {
    #[key]
    #[auto(increment)]
    id: uuid::Uuid,
    title: String,
}

fn main() {}
