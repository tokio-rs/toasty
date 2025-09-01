use super::plan;
use crate::engine::ExecResponse;

#[derive(Debug)]
pub(crate) struct VarStore {
    slots: Vec<Option<ExecResponse>>,
}

impl VarStore {
    pub(crate) fn new() -> Self {
        Self { slots: vec![] }
    }

    pub(crate) fn load(&mut self, var: plan::VarId) -> ExecResponse {
        let Some(response) = self.slots[var.0].take() else {
            panic!("no response at slot {}; store={:#?}", var.0, self);
        };

        response
    }

    pub(crate) async fn dup(&mut self, var: plan::VarId) -> crate::Result<ExecResponse> {
        let Some(response) = &mut self.slots[var.0] else {
            panic!("no response at slot {}; store={:#?}", var.0, self);
        };

        // Duplicate the entire ExecResponse, including metadata
        let values = response.values.dup().await?;
        Ok(ExecResponse {
            values,
            metadata: response.metadata.clone(),
        })
    }

    pub(crate) fn store(&mut self, var: plan::VarId, response: ExecResponse) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        self.slots[var.0] = Some(response);
    }
}
