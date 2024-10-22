use super::*;

#[derive(Debug)]
pub(crate) struct Path {
    pub segments: Punctuated<PathSegment, punct::PathSep>,
}

#[derive(Debug)]
pub(crate) struct PathSegment {
    pub ident: Ident,
    pub arguments: Option<PathArguments>,
}

#[derive(Debug)]
pub(crate) struct PathArguments {
    pub arguments: AngleBracketed<Type, punct::Comma>,
}

impl Path {
    pub(crate) fn new(ident: &Ident) -> Path {
        Path {
            segments: Punctuated {
                items: vec![(
                    PathSegment {
                        ident: ident.clone(),
                        arguments: None,
                    },
                    None,
                )],
            },
        }
    }
}

impl Parse for Path {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Path {
            segments: p.parse()?,
        })
    }
}

impl Parse for PathSegment {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(PathSegment {
            ident: p.parse()?,
            arguments: if p.is_next::<punct::Lt>() {
                Some(p.parse()?)
            } else {
                None
            },
        })
    }
}

impl Parse for PathArguments {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(PathArguments {
            arguments: p.parse()?,
        })
    }
}
