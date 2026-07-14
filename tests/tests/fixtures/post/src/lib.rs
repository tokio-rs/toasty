#[derive(Debug, toasty::Model)]
pub struct Post {
    #[key]
    #[auto]
    id: u64,
    title: String,
}
