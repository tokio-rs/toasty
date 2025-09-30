use crate::{
    engine::{exec::Exec, plan},
    Result,
};

impl Exec<'_> {
    pub(super) async fn action_project(&mut self, action: &plan::Project) -> Result<()> {
        todo!("action={action:#?}");
    }
}
