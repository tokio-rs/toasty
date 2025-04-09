use super::Query;

#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    pub query: Query,
}
