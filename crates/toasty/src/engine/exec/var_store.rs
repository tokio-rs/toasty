use super::plan;
use toasty_core::stmt::{self, ValueStream};

#[derive(Debug)]
pub(crate) struct VarStore {
    slots: Vec<Option<Entry>>,
    tys: Vec<stmt::Type>,
}

#[derive(Debug)]
struct Entry {
    value_stream: ValueStream,
    count: usize,
}

impl VarStore {
    pub(crate) fn new(tys: Vec<stmt::Type>) -> Self {
        Self { slots: vec![], tys }
    }

    pub(crate) fn load(&mut self, var: plan::VarId) -> ValueStream {
        let Some(entry) = self.slots[var.0].take() else {
            panic!("no stream at slot {}; store={:#?}", var.0, self);
        };

        debug_assert_eq!(entry.count, 1);

        entry.value_stream
    }

    pub(crate) async fn load_count(&mut self, var: plan::VarId) -> crate::Result<ValueStream> {
        let Some(entry) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self)
        };

        if entry.count == 1 {
            return Ok(self.slots[var.0].take().unwrap().value_stream);
        }

        entry.count -= 1;
        entry.value_stream.dup().await
    }

    #[track_caller]
    pub(crate) fn store(&mut self, var: plan::VarId, stream: ValueStream) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let stmt::Type::List(item_tys) = &self.tys[var.0] else {
            todo!()
        };
        self.slots[var.0] = Some(Entry {
            value_stream: stream.typed((**item_tys).clone()),
            count: 1,
        });
    }

    #[track_caller]
    pub(crate) fn store_counted(&mut self, var: plan::VarId, count: usize, stream: ValueStream) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let stmt::Type::List(item_tys) = &self.tys[var.0] else {
            todo!()
        };
        self.slots[var.0] = Some(Entry {
            value_stream: stream.typed((**item_tys).clone()),
            count,
        });
    }
}
