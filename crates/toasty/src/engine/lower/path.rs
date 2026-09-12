//! Application-path traversal used during lowering.

use toasty_core::stmt::{Expr, PathStep};

pub(super) fn flatten<'a>(expr: &'a Expr, steps: &mut Vec<PathStep>) -> &'a Expr {
    match expr {
        Expr::Path(path) => {
            let base = flatten(&path.base, steps);
            steps.extend_from_slice(&path.steps);
            base
        }
        Expr::Project(project) => {
            let base = flatten(&project.base, steps);
            steps.extend(project.projection.iter().copied().map(PathStep::Field));
            base
        }
        _ => expr,
    }
}
