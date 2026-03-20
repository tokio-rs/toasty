use toasty_core::{driver::Rows, stmt};

use super::ExecResponse;

/// Tracks variable declarations during planning. Each variable has a type and
/// is assigned a unique VarId. This is converted into a VarStore for execution.
#[derive(Debug, Default)]
pub(crate) struct VarDecls {
    /// Variable types
    vars: Vec<stmt::Type>,
}

impl VarDecls {
    #[track_caller]
    pub(crate) fn register_var(&mut self, ty: stmt::Type) -> VarId {
        // Register a new slot
        let ret = self.vars.len();
        self.vars.push(ty);
        VarId(ret)
    }
}

#[derive(Debug)]
pub(crate) struct VarStore {
    slots: Vec<Option<Entry>>,
    tys: Vec<stmt::Type>,
}

/// Identifies a pipeline variable slot
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct VarId(pub(crate) usize);

#[derive(Debug)]
struct Entry {
    response: ExecResponse,
    count: usize,
}

impl VarStore {
    pub(crate) fn new(decls: VarDecls) -> Self {
        Self {
            slots: vec![],
            tys: decls.vars,
        }
    }

    pub(crate) async fn load(&mut self, var: VarId) -> crate::Result<Rows> {
        let Some(entry) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self)
        };

        tracing::debug!(
            "VarStore::load({:?}): remaining uses = {}",
            var,
            entry.count
        );

        if entry.count == 1 {
            return Ok(self.slots[var.0].take().unwrap().response.values);
        }

        entry.count -= 1;
        entry.response.values.dup().await
    }

    #[track_caller]
    pub(crate) fn store(&mut self, var: VarId, count: usize, response: ExecResponse) {
        tracing::debug!(
            "VarStore::store({:?}): uses = {}, type = {:?}",
            var,
            count,
            self.tys.get(var.0)
        );

        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let values = match response.values {
            Rows::Count(_) => {
                assert!(self.tys[var.0].is_unit());
                response.values
            }
            Rows::Value(value) => {
                assert!(
                    value.is_a(&self.tys[var.0]),
                    "type mismatch: {value:?} is not a {:?}",
                    self.tys[var.0]
                );
                Rows::Value(value)
            }
            Rows::Stream(value_stream) => {
                let stmt::Type::List(item_tys) = &self.tys[var.0] else {
                    todo!("ty={:#?}", self.tys[var.0])
                };

                Rows::Stream(value_stream.typed((**item_tys).clone()))
            }
        };

        let response = ExecResponse {
            values,
            next_cursor: response.next_cursor,
            prev_cursor: response.prev_cursor,
        };

        self.slots[var.0] = Some(Entry { response, count });
    }
}
