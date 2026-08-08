// Field-level `#[index]` must not take arguments. A composite index that
// spans multiple fields belongs on the struct, not on a single field.

#[derive(toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index(other)]
    foo: uuid::Uuid,

    other: uuid::Uuid,
}

fn main() {}
