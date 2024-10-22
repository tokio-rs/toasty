use super::*;

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Ident(Ident),
}

impl Parse for Expr {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        if p.is_next::<Ident>() {
            p.parse().map(Expr::Ident)
        } else {
            todo!()
        }
    }
}
