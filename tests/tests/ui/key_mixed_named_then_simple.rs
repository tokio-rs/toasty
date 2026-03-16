#[derive(toasty::Model)]
#[key(partition = name, local = id, extra)]
struct Widget {
    id: uuid::Uuid,
    name: String,
    extra: String,
}

fn main() {}
