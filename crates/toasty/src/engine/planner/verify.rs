use super::*;

impl Planner<'_> {
    pub(super) fn verify_action(&self, action: &plan::Action) {
        use plan::Action::*;

        match action {
            Associate(action) => self.verify_associate(action),
            BatchWrite(action) => self.verify_batch_write(action),
            DeleteByKey(action) => self.verify_delete_by_key(action),
            ExecStatement(action) => self.verify_exec_statement(action),
            FindPkByIndex(action) => self.verify_find_pk_by_index(action),
            GetByKey(action) => self.verify_get_by_key(action),
            Insert(action) => self.verify_insert(action),
            QueryPk(action) => self.verify_query_pk(action),
            ReadModifyWrite(action) => self.verify_read_modify_write(action),
            SetVar(action) => self.verify_set_var(action),
            UpdateByKey(action) => self.verify_update_by_key(action),
        }
    }

    pub(crate) fn verify_associate(&self, _action: &plan::Associate) {}

    pub(super) fn verify_write_action(&self, action: &plan::WriteAction) {
        use plan::WriteAction::*;

        match action {
            DeleteByKey(action) => self.verify_delete_by_key(action),
            Insert(action) => self.verify_insert(action),
            UpdateByKey(action) => self.verify_update_by_key(action),
        }
    }

    fn verify_batch_write(&self, action: &plan::BatchWrite) {
        for action in &action.items {
            self.verify_write_action(action);
        }
    }

    fn verify_delete_by_key(&self, _action: &plan::DeleteByKey) {}

    fn verify_find_pk_by_index(&self, _action: &plan::FindPkByIndex) {}

    fn verify_get_by_key(&self, _action: &plan::GetByKey) {}

    fn verify_insert(&self, _action: &plan::Insert) {}

    fn verify_query_pk(&self, _action: &plan::QueryPk) {}

    fn verify_exec_statement(&self, _action: &plan::ExecStatement) {}

    fn verify_read_modify_write(&self, _action: &plan::ReadModifyWrite) {}

    fn verify_set_var(&self, _action: &plan::SetVar) {}

    fn verify_update_by_key(&self, _action: &plan::UpdateByKey) {}
}
