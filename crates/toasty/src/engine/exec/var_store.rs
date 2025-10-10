use super::plan;
use toasty_core::stmt::{self, ValueStream};

#[derive(Debug)]
pub(crate) struct VarStore {
    slots: Vec<Option<ValueStream>>,
    tys: Vec<stmt::Type>,
}

impl VarStore {
    pub(crate) fn new(tys: Vec<stmt::Type>) -> Self {
        Self { slots: vec![], tys }
    }

    pub(crate) fn load(&mut self, var: plan::VarId) -> ValueStream {
        let Some(stream) = self.slots[var.0].take() else {
            panic!("no stream at slot {}; store={:#?}", var.0, self);
        };

        stream
    }

    pub(crate) async fn dup(&mut self, var: plan::VarId) -> crate::Result<ValueStream> {
        let Some(stream) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self);
        };

        stream.dup().await
    }

    #[track_caller]
    pub(crate) fn store(&mut self, var: plan::VarId, stream: ValueStream) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let stmt::Type::List(item_tys) = &self.tys[var.0] else {
            todo!()
        };
        self.slots[var.0] = Some(stream.typed((**item_tys).clone()));
    }
}
