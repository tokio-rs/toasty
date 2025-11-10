use super::plan;
use toasty_core::{driver::Rows, stmt};

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

    pub(crate) async fn load(&mut self, var: plan::VarId) -> crate::Result<Rows> {
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
    pub(crate) fn store(&mut self, var: plan::VarId, count: usize, rows: Rows) {
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
                    todo!("ty={:#?}", self.tys[var.0])
                };

                Rows::Values(value_stream.typed((**item_tys).clone()))
            }
        };

        self.slots[var.0] = Some(Entry { rows, count });
    }
}
