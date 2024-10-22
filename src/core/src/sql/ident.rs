use super::*;

#[derive(Debug, Clone)]
pub struct Ident(pub String);

impl From<&str> for Ident {
    fn from(value: &str) -> Self {
        Ident(value.into())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
