use std::fmt;

#[derive(Copy, Clone)]
pub enum SetOp {
    Union,
    Except,
    Intersect,
}

impl SetOp {}

impl fmt::Display for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetOp::Union => "UNION".fmt(f),
            SetOp::Except => "EXCEPT".fmt(f),
            SetOp::Intersect => "INTERSECT".fmt(f),
        }
    }
}

impl fmt::Debug for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
