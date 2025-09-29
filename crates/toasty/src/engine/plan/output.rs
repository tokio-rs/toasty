use toasty_core::stmt::Type;

use super::{eval, VarId};

#[derive(Debug, Clone)]
pub(crate) struct Output {
    /// The Toasty-level type returned by the database. When `None`, then number
    /// of rows impacted is returned.
    pub ty: Option<Vec<Type>>,

    /// What to do with the output. This may end up being fanned out to multiple
    /// variables.
    pub targets: Vec<OutputTarget>,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputTarget {
    /// Where to store the output
    pub var: VarId,

    /// How to project it before storing
    pub project: eval::Func,
}

impl Output {
    #[track_caller]
    pub fn single_target(var: VarId, project: eval::Func) -> Output {
        Output {
            ty: todo!("update call to use single_target2"),
            targets: vec![OutputTarget { var, project }],
        }
    }

    pub fn single_target2(var: VarId, project: eval::Func) -> Output {
        Output {
            ty: if project.args.is_empty() {
                None
            } else {
                Some(project.args.clone())
            },
            targets: vec![OutputTarget { var, project }],
        }
    }
}
