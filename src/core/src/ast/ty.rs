use super::*;

#[derive(Debug)]
pub(crate) enum Type {
    /// An array of types
    Array(TypeArray),

    /// A type path
    Path(TypePath),

    /// An optional type (nullable)
    Option(TypeOption),
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct TypeArray {
    pub l_bracket: LBracket,
    pub ty: Box<Type>,
    pub r_bracket: RBracket,
}

#[derive(Debug)]
pub(crate) struct TypePath {
    /// Type identifier
    pub path: Path,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct TypeOption {
    /// The `Option` identifier
    pub kw_option: keyword::Opt,

    pub l_angle: Lt,

    pub ty: Box<Type>,

    pub r_angle: Gt,
}

impl Parse for Type {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        if p.is_next::<LBracket>() {
            p.parse().map(Type::Array)
        } else if p.is_next::<keyword::Opt>() {
            p.parse().map(Type::Option)
        } else {
            p.parse().map(Type::Path)
        }
    }
}

impl Parse for TypeArray {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(TypeArray {
            l_bracket: p.parse()?,
            ty: Box::new(p.parse()?),
            r_bracket: p.parse()?,
        })
    }
}

impl Parse for TypeOption {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(TypeOption {
            kw_option: p.parse()?,
            l_angle: p.parse()?,
            ty: p.parse()?,
            r_angle: p.parse()?,
        })
    }
}

impl Parse for TypePath {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(TypePath { path: p.parse()? })
    }
}
