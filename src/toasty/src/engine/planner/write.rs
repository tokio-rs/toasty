use super::*;

pub(super) struct WritePlanner<'stmt> {
    capability: &'stmt Capability,
    schema: &'stmt Schema,
}