use std::collections::HashMap;

use super::Schema;

pub struct DiffContext<'a> {
    from: &'a Schema,
    to: &'a Schema,

    renamed_tables: HashMap<&'a str, &'a str>,
    renamed_columns: HashMap<&'a str, &'a str>,
    renamed_indices: HashMap<&'a str, &'a str>,
}
