use super::Cte;

#[derive(Debug, Clone, PartialEq)]
pub struct With {
    pub ctes: Vec<Cte>,
}
