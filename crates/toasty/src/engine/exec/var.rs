use std::sync::Arc;

use index_vec::IndexVec;
use toasty_core::{
    driver::{ExecResponse, Rows},
    schema::Schema,
    stmt,
};

use crate::engine::mir::NodeId;

/// Runtime storage for node outputs: one slot per MIR node, keyed by the
/// node's [`NodeId`].
#[derive(Debug)]
pub(crate) struct VarStore {
    slots: IndexVec<NodeId, Option<Entry>>,
    /// Resolves `Type::Model` (`#[document]`) layouts for the value type-checks.
    schema: Arc<Schema>,
}

#[derive(Debug)]
struct Entry {
    response: ExecResponse,
    count: usize,
}

impl VarStore {
    pub(crate) fn new(node_count: usize, schema: Arc<Schema>) -> Self {
        Self {
            slots: IndexVec::from_vec((0..node_count).map(|_| None).collect()),
            schema,
        }
    }

    pub(crate) async fn load(&mut self, node: NodeId) -> crate::Result<ExecResponse> {
        let Some(entry) = &mut self.slots[node] else {
            panic!("no stream at slot {node:?}; store={self:#?}")
        };

        if entry.count == 1 {
            return Ok(self.slots[node].take().unwrap().response);
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
    /// counting expects (a skipped `If` arm).
    #[track_caller]
    pub(crate) fn release(&mut self, node: NodeId) {
        let Some(entry) = self.slots[node].as_mut() else {
            panic!("release of unset slot {node:?}; store={self:#?}")
        };

        if entry.count == 1 {
            self.slots[node] = None;
        } else {
            entry.count -= 1;
        }
    }

    /// Returns whether the slot holds at least one row, without consuming a
    /// use. A stream-backed slot is buffered in place so the peek does not
    /// disturb later loads.
    pub(crate) async fn peek_non_empty(&mut self, node: NodeId) -> crate::Result<bool> {
        let Some(entry) = self.slots[node].as_mut() else {
            panic!("no stream at slot {node:?}; store={self:#?}")
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

    /// Assigns a slot the empty value of its type: a `List` becomes an empty
    /// list, anything else `Null`. Used for the escaping outputs of a
    /// skipped `If` arm.
    pub(crate) fn store_empty(&mut self, node: NodeId, ty: &stmt::Type, count: usize) {
        let value = match ty {
            stmt::Type::List(_) => stmt::Value::List(vec![]),
            _ => stmt::Value::Null,
        };
        self.store(node, ty, count, ExecResponse::from_rows(Rows::Value(value)));
    }

    #[track_caller]
    pub(crate) fn store(
        &mut self,
        node: NodeId,
        ty: &stmt::Type,
        count: usize,
        response: ExecResponse,
    ) {
        // A zero-use output is never observed; don't occupy a slot.
        if count == 0 {
            return;
        }

        let values = match response.values {
            Rows::Count(_) => {
                assert!(ty.is_unit());
                response.values
            }
            Rows::Value(value) => {
                assert!(
                    value.is_a(&self.schema.app, ty),
                    "type mismatch: {value:?} is not a {ty:?}",
                );
                Rows::Value(value)
            }
            Rows::Stream(value_stream) => {
                let stmt::Type::List(item_tys) = ty else {
                    todo!("ty={ty:#?}")
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

        self.slots[node] = Some(Entry { response, count });
    }
}
