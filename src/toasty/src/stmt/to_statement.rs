use super::*;

pub trait ToStatement<'a> {
    type Model;

    fn to_statement(self) -> Statement<'a, Self::Model>;
}
