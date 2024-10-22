use std::fmt;

#[derive(Copy, Clone, PartialEq)]
pub enum SetOp {
    Union,
    Except,
    Intersect,
}

impl SetOp {}

impl fmt::Display for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SetOp::*;

        match self {
            Union => "UNION".fmt(f),
            Except => "EXCEPT".fmt(f),
            Intersect => "INTERSECT".fmt(f),
        }
    }
}

impl fmt::Debug for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
