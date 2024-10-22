use super::*;

#[derive(Debug)]
pub(crate) struct Schema {
    pub items: Vec<SchemaItem>,
}

#[derive(Debug)]
pub(crate) enum SchemaItem {
    Model(Model),
    Table(Table),
}

impl Parse for Schema {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Schema {
            items: p.parse_repeated_until::<_, Eof>()?,
        })
    }
}

impl Parse for SchemaItem {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        let mut attrs = vec![];

        loop {
            if p.is_next::<keyword::Model>() {
                let mut model = p.parse::<Model>()?;
                model.attrs = attrs;

                return Ok(SchemaItem::Model(model));
            } else if p.is_next::<keyword::Table>() {
                return p.parse().map(SchemaItem::Table);
            } else if p.is_next::<Pound>() {
                attrs.push(p.parse::<Attribute>()?);
            } else {
                todo!("{:?}", p.parse::<Token>()?);
            }
        }
    }
}

impl Schema {
    pub(crate) fn models(&self) -> impl Iterator<Item = &'_ Model> + '_ {
        self.items
            .iter()
            .flat_map(|item| -> Box<dyn Iterator<Item = &Model>> {
                match item {
                    SchemaItem::Model(model) => Box::new(Some(model).into_iter()),
                    SchemaItem::Table(table) => Box::new(table.models.iter()),
                }
            })
    }
}
