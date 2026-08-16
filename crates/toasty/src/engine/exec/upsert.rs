use toasty_core::{
    driver::{ExecResponse, operation},
    stmt,
};

use crate::{
    Result,
    engine::{exec::Exec, mir},
};

impl Exec<'_> {
    pub(super) async fn exec_upsert(&mut self, action: &mir::Upsert) -> Result<ExecResponse> {
        let mut stmt = stmt::Statement::from(action.stmt.clone());

        if !action.inputs.is_empty() {
            let input = self.collect_input(action.inputs.iter().copied()).await?;
            stmt.substitute(&input);
            self.engine.simplify_stmt(&mut stmt);
        }

        let params = self.engine.prepare_for_driver(&mut stmt);
        let op = operation::Upsert {
            stmt: stmt.into_insert_unwrap(),
            params,
            ret: mir::row_field_types(&action.ty),
        };

        self.connection.exec(&self.engine.schema, op.into()).await
    }
}
