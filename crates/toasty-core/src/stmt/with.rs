use super::Cte;

#[derive(Debug, Clone)]
pub struct With {
    pub ctes: Vec<Cte>,
}
