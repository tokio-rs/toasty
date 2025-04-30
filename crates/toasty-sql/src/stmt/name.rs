use std::fmt;

#[derive(Debug, Clone)]
pub struct Name(pub Vec<String>);

impl From<&str> for Name {
    fn from(value: &str) -> Self {
        Self(vec![value.into()])
    }
}

impl From<&String> for Name {
    fn from(value: &String) -> Self {
        Self::from(&value[..])
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = "";
        for ident in &self.0 {
            write!(f, "{s}{ident}")?;
            s = ", ";
        }

        Ok(())
    }
}
