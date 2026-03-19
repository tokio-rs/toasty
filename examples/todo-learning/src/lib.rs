#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    pub name: String,

    #[unique]
    pub email: String,
}
