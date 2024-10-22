use super::*;

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct Model {
    pub attrs: Vec<Attribute>,
    pub kw_model: keyword::Model,
    pub ident: Ident,
    pub l_brace: LBrace,
    pub fields: Vec<Field>,
    pub r_brace: RBrace,
}

impl Parse for Model {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Model {
            attrs: vec![],
            kw_model: p.parse()?,
            ident: p.parse()?,
            l_brace: p.parse()?,
            fields: p.parse_repeated_until::<_, RBrace>()?,
            r_brace: p.parse()?,
        })
    }
}
