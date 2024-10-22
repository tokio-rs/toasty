use super::*;

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct Table {
    pub kw_table: keyword::Table,
    pub ident: Ident,
    pub l_brace: LBrace,
    pub models: Vec<Model>,
    pub r_brace: RBrace,
}

impl Parse for Table {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Table {
            kw_table: p.parse()?,
            ident: p.parse()?,
            l_brace: p.parse()?,
            models: parse_body(p)?,
            r_brace: p.parse()?,
        })
    }
}

fn parse_body(p: &mut Parser<'_>) -> Result<Vec<Model>> {
    let mut attrs = vec![];
    let mut models = vec![];

    loop {
        if p.is_next::<keyword::Model>() {
            let mut model: Model = p.parse()?;
            model.attrs = std::mem::take(&mut attrs);
            models.push(model);
        } else if p.is_next::<Pound>() {
            attrs.push(p.parse()?);
        } else {
            return Ok(models);
        }
    }
}
