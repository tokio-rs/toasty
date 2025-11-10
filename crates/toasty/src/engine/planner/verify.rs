use crate::engine::{plan, planner::Planner};

impl Planner<'_> {
    pub(super) fn verify_action(&self, _action: &plan::Action) {}
}
