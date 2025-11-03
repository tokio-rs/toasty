use super::plan;
use toasty_core::{
    driver::Rows,
    stmt::{self, ValueStream},
};

#[derive(Debug)]
pub(crate) struct VarStore {
    slots: Vec<Option<Entry>>,
    tys: Vec<stmt::Type>,
}

#[derive(Debug)]
struct Entry {
    rows: Rows,
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

        entry.rows.into_values()
    }

    pub(crate) async fn load_count(&mut self, var: plan::VarId) -> crate::Result<Rows> {
        let Some(entry) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self)
        };

        if entry.count == 1 {
            return Ok(self.slots[var.0].take().unwrap().rows);
        }

        entry.count -= 1;
        entry.rows.dup().await
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
            rows: Rows::Values(stream.typed((**item_tys).clone())),
            count: 1,
        });
    }

    #[track_caller]
    pub(crate) fn store_counted(&mut self, var: plan::VarId, count: usize, rows: Rows) {
        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let rows = match rows {
            Rows::Count(_) => {
                assert!(self.tys[var.0].is_unit());
                rows
            }
            Rows::Values(value_stream) => {
                let stmt::Type::List(item_tys) = &self.tys[var.0] else {
                    todo!()
                };

                Rows::Values(value_stream.typed((**item_tys).clone()))
            }
        };

        self.slots[var.0] = Some(Entry { rows, count });
    }
}
