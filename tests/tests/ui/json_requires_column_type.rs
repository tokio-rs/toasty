// The derive must resolve aliases and transparent field wrappers through the
// `Field` trait instead of inspecting the field's type syntax.

type Payload = toasty::Json<Vec<String>>;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: i64,

    payload: Option<Payload>,
}

fn main() {}
