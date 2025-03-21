use super::*;

pub trait ToStatement {
    type Model;

    fn to_statement(self) -> Statement<Self::Model>;
}
