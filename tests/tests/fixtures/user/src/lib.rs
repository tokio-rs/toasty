#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    id: u64,
    name: String,
}
