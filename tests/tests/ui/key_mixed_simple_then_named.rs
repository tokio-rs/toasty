#[derive(toasty::Model)]
#[key(id, partition = name)]
struct Widget {
    id: uuid::Uuid,
    name: String,
}

fn main() {}
