#[derive(Debug, toasty::Model)]
pub struct Item {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    #[index]
    pub order: i64,
}
