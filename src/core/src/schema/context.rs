use super::*;

use std::collections::HashMap;

pub(crate) struct Context {
    /// Maps table names to identifiers
    table_lookup: HashMap<String, TableId>,
}

impl Context {
    pub(crate) fn new() -> Context {
        Context {
            table_lookup: HashMap::new(),
        }
    }

    pub(crate) fn register_table(&mut self, name: impl AsRef<str>) -> table::TableId {
        assert!(!self.table_lookup.contains_key(name.as_ref()));
        let id = table::TableId(self.table_lookup.len());
        self.table_lookup.insert(name.as_ref().to_string(), id);
        id
    }
}
