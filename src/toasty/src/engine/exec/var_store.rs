use toasty_core::stmt::ValueStream;

use super::*;

#[derive(Debug)]
pub(crate) struct VarStore<'stmt> {
    slots: Vec<Option<ValueStream<'stmt>>>,
}

impl<'stmt> VarStore<'stmt> {
    pub(crate) fn new() -> VarStore<'stmt> {
        VarStore { slots: vec![] }
    }

    pub(crate) fn load(&mut self, var: plan::VarId) -> ValueStream<'stmt> {
        let Some(stream) = self.slots[var.0].take() else {
            panic!("no stream at slot {}; store={:#?}", var.0, self);
        };

        stream
    }

    pub(crate) async fn dup(&mut self, var: plan::VarId) -> crate::Result<ValueStream<'stmt>> {
        let Some(stream) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self);
        };

        stream.dup().await
    }

    pub(crate) fn store(&mut self, var: plan::VarId, stream: ValueStream<'stmt>) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        self.slots[var.0] = Some(stream);
    }
}
