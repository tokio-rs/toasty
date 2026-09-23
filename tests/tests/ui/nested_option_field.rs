type OptionalText = Option<String>;

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: uuid::Uuid,
    value: Option<Box<OptionalText>>,
}

fn main() {
    let _ = <Item as toasty::schema::Model>::schema();
}
