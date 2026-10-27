#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
    parent_id: i64,
    #[belongs_to(key = parent_id)]
    parent: toasty::Deferred<Parent>,
    #[has_one(via = parent)]
    other: toasty::Deferred<Option<Other>>,
}

#[derive(Debug, toasty::Model)]
struct Parent {
    #[key]
    id: i64,
}

#[derive(Debug, toasty::Model)]
struct Other {
    #[key]
    id: i64,
}

fn main() {}
