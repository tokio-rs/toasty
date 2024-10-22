use super::*;

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct Field {
    pub attrs: Vec<Attribute>,
    pub ident: Ident,
    pub colon: Colon,
    pub ty: Type,
    pub comma: Comma,
}

impl Parse for Field {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Field {
            attrs: p.parse_repeated_while::<_, Pound>()?,
            ident: p.parse()?,
            colon: p.parse()?,
            ty: p.parse()?,
            comma: p.parse()?,
        })
    }
}
