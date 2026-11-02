use toasty::Model;

#[derive(Model)]
#[comment = "Use #[table(comment = ...)] instead"]
struct User {
    #[key]
    id: i64,

    #[comment = "Use #[column(comment = ...)] instead"]
    name: String,
}

fn main() {}
