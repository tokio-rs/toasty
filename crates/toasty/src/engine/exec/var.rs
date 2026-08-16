use std::sync::Arc;
use toasty_core::{
    driver::{ExecResponse, Rows},
    schema::Schema,
    stmt,
};

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
    /// Resolves `Type::Model` (`#[document]`) layouts for the value type-checks.
    schema: Arc<Schema>,
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
    pub(crate) fn new(decls: VarDecls, schema: Arc<Schema>) -> Self {
        Self {
            slots: vec![],
            tys: decls.vars,
            schema,
        }
    }

    pub(crate) async fn load(&mut self, var: VarId) -> crate::Result<ExecResponse> {
        let Some(entry) = &mut self.slots[var.0] else {
            panic!("no stream at slot {}; store={:#?}", var.0, self)
        };

        if entry.count == 1 {
            return Ok(self.slots[var.0].take().unwrap().response);
        }

        entry.count -= 1;
        Ok(ExecResponse {
            values: entry.response.values.dup().await?,
            next_cursor: entry.response.next_cursor.clone(),
            prev_cursor: entry.response.prev_cursor.clone(),
        })
    }

    /// Decrements a slot's use count without observing its value, dropping
    /// the entry at zero. Called on paths that decline a load the use
    /// counting expects (an `If` else arm).
    #[track_caller]
    pub(crate) fn release(&mut self, var: VarId) {
        let Some(entry) = self.slots.get_mut(var.0).and_then(Option::as_mut) else {
            panic!("release of unset slot {}; store={:#?}", var.0, self)
        };

        if entry.count == 1 {
            self.slots[var.0] = None;
        } else {
            entry.count -= 1;
        }
    }

    /// Returns whether the slot holds at least one row, without consuming a
    /// use. A stream-backed slot is buffered in place so the peek does not
    /// disturb later loads.
    pub(crate) async fn peek_non_empty(&mut self, var: VarId) -> crate::Result<bool> {
        let Some(entry) = self.slots.get_mut(var.0).and_then(Option::as_mut) else {
            panic!("no stream at slot {}; store={:#?}", var.0, self)
        };

        entry.response.values.buffer().await?;

        Ok(match &entry.response.values {
            Rows::Count(count) => *count > 0,
            Rows::Value(stmt::Value::List(items)) => !items.is_empty(),
            Rows::Value(stmt::Value::Null) => false,
            Rows::Value(_) => true,
            Rows::Stream(_) => unreachable!("stream was buffered above"),
        })
    }

    /// Debug-asserts every slot has been drained. With exact use counts this
    /// holds after the final load of the plan's returning variable, on the
    /// success path only — a mid-plan failure legitimately leaves loads
    /// unperformed. Undercounting already panics loudly on a load of a freed
    /// slot; this converts the silent overcounting direction into a loud one.
    pub(crate) fn assert_empty(&self) {
        debug_assert!(
            self.slots.iter().all(Option::is_none),
            "variable slots not drained at plan completion; store={self:#?}"
        );
    }

    /// Assigns a slot the empty value of its declared type: a `List` becomes
    /// an empty list, anything else `Null`. Used for the escaping outputs of
    /// a skipped `If` arm.
    pub(crate) fn store_empty(&mut self, var: VarId, count: usize) {
        let value = match &self.tys[var.0] {
            stmt::Type::List(_) => stmt::Value::List(vec![]),
            _ => stmt::Value::Null,
        };
        self.store(var, count, ExecResponse::from_rows(Rows::Value(value)));
    }

    #[track_caller]
    pub(crate) fn store(&mut self, var: VarId, count: usize, response: ExecResponse) {
        // A zero-use output is never observed; don't occupy a slot.
        if count == 0 {
            return;
        }

        while self.slots.len() <= var.0 {
            self.slots.push(None);
        }

        let values = match response.values {
            Rows::Count(_) => {
                assert!(self.tys[var.0].is_unit());
                response.values
            }
            Rows::Value(value) => {
                let ty = &self.tys[var.0];
                assert!(
                    value.is_a(&self.schema.app, ty),
                    "type mismatch: {value:?} is not a {ty:?}",
                );
                Rows::Value(value)
            }
            Rows::Stream(value_stream) => {
                let stmt::Type::List(item_tys) = &self.tys[var.0] else {
                    todo!("ty={:#?}", self.tys[var.0])
                };
                let item_ty = (**item_tys).clone();

                Rows::Stream(value_stream.typed(self.schema.clone(), item_ty))
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
