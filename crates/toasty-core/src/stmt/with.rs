use super::Cte;

#[derive(Debug, Clone)]
pub struct With {
    pub ctes: Vec<Cte>,
}

impl From<Vec<Cte>> for With {
    fn from(ctes: Vec<Cte>) -> Self {
        Self { ctes }
    }
}
