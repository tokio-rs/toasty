#![allow(dead_code)] // TODO: remove
use super::*;

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    pub pound: Pound,
    pub l_bracket: LBracket,
    pub meta: Meta,
    pub r_bracket: RBracket,
}

#[derive(Debug, Clone)]
pub(crate) enum Meta {
    Ident(Ident),
    List(MetaList),
    NameValue(MetaNameValue),
}

#[derive(Debug, Clone)]
pub(crate) struct MetaList {
    pub ident: Ident,
    pub items: Parenthesized<Meta, Comma>,
}

#[derive(Debug, Clone)]
pub(crate) struct MetaNameValue {
    pub name: Ident,
    pub eq: Eq,
    pub value: Expr,
}

impl Meta {
    pub(crate) fn ident(&self) -> &Ident {
        match self {
            Meta::Ident(ident) => ident,
            Meta::List(list) => &list.ident,
            Meta::NameValue(name_value) => &name_value.name,
        }
    }

    pub(crate) fn as_list(&self) -> &MetaList {
        match self {
            Meta::List(list) => list,
            _ => todo!(),
        }
    }
}

impl Parse for Attribute {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Attribute {
            pound: p.parse()?,
            l_bracket: p.parse()?,
            meta: p.parse()?,
            r_bracket: p.parse()?,
        })
    }
}

impl Parse for Meta {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        if p.is_nth::<Eq>(1) {
            p.parse().map(Meta::NameValue)
        } else if p.is_nth::<LParen>(1) {
            p.parse().map(Meta::List)
        } else {
            p.parse().map(Meta::Ident)
        }
    }
}

impl Parse for MetaList {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(MetaList {
            ident: p.parse()?,
            items: p.parse()?,
        })
    }
}

impl Parse for MetaNameValue {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(MetaNameValue {
            name: p.parse()?,
            eq: p.parse()?,
            value: p.parse()?,
        })
    }
}
