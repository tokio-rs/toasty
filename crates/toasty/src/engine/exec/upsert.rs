use toasty_core::{driver::operation, stmt};

use crate::{
    Result,
    engine::exec::{Action, Exec, Output, VarId},
};

/// Executes a lowered single-row upsert on a non-SQL database.
#[derive(Debug)]
pub(crate) struct Upsert {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// Where to store the result.
    pub output: Output,

    /// The lowered insert and conflict action.
    pub stmt: stmt::Insert,

    /// Types of columns returned by the operation.
    pub ret: Option<Vec<stmt::Type>>,
}

impl Exec<'_> {
    pub(super) async fn action_upsert(&mut self, action: &Upsert) -> Result<()> {
        let mut stmt = stmt::Statement::from(action.stmt.clone());

        if !action.input.is_empty() {
            let input = self.collect_input(&action.input).await?;
            stmt.substitute(&input);
            self.engine.simplify_stmt(&mut stmt);
        }

        let params = self.engine.prepare_for_driver(&mut stmt);
        let op = operation::Upsert {
            stmt: stmt.into_insert_unwrap(),
            params,
            ret: action.ret.clone(),
        };

        let res = self.connection.exec(&self.engine.schema, op.into()).await?;
        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}

impl From<Upsert> for Action {
    fn from(value: Upsert) -> Self {
        Self::Upsert(value)
    }
}
