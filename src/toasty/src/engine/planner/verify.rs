use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(super) fn verify_action(&self, action: &plan::Action<'stmt>) {
        use plan::Action::*;

        match action {
            Associate(action) => self.verify_associate(action),
            BatchWrite(action) => self.verify_batch_write(action),
            DeleteByKey(action) => self.verify_delete_by_key(action),
            FindPkByIndex(action) => self.verify_find_pk_by_index(action),
            GetByKey(action) => self.verify_get_by_key(action),
            Insert(action) => self.verify_insert(action),
            QueryPk(action) => self.verify_query_pk(action),
            QuerySql(action) => self.verify_query_sql(action),
            UpdateByKey(action) => self.verify_update_by_key(action),
            SetVar(action) => self.verify_set_var(action),
        }
    }

    pub(crate) fn verify_associate(&self, _action: &plan::Associate) {}

    pub(super) fn verify_write_action(&self, action: &plan::WriteAction<'stmt>) {
        use plan::WriteAction::*;

        match action {
            DeleteByKey(action) => self.verify_delete_by_key(action),
            Insert(action) => self.verify_insert(action),
            UpdateByKey(action) => self.verify_update_by_key(action),
        }
    }

    fn verify_batch_write(&self, action: &plan::BatchWrite<'stmt>) {
        for action in &action.items {
            self.verify_write_action(action);
        }
    }

    fn verify_delete_by_key(&self, _action: &plan::DeleteByKey<'stmt>) {}

    fn verify_find_pk_by_index(&self, _action: &plan::FindPkByIndex<'stmt>) {}

    fn verify_get_by_key(&self, _action: &plan::GetByKey<'stmt>) {}

    fn verify_insert(&self, _action: &plan::Insert<'stmt>) {}

    fn verify_query_pk(&self, _action: &plan::QueryPk<'stmt>) {}

    fn verify_query_sql(&self, _action: &plan::QuerySql<'stmt>) {}

    fn verify_update_by_key(&self, _action: &plan::UpdateByKey<'stmt>) {}

    fn verify_set_var(&self, _action: &plan::SetVar<'stmt>) {}
}
