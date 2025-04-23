use super::{plan, Exec};

use crate::Result;

impl Exec<'_> {
    pub(super) async fn action_read_modify_write(
        &mut self,
        action: &plan::ReadModifyWrite,
    ) -> Result<()> {
        todo!()
    }
}
