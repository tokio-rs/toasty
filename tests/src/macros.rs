#[macro_export]
macro_rules! assert_eq_unordered {
    ($actual:expr, $expect:expr) => {
        let mut vals = std::collections::HashSet::new();

        for val in $actual {
            assert!(vals.insert(val));
        }

        for val in $expect {
            assert!(vals.remove(val), "`{:#?}` missing", val);
        }

        assert!(vals.is_empty());
    };
}

#[macro_export]
macro_rules! columns {
    ($db:expr, $table_name:expr, [$($col:expr),* $(,)?]) => {{
        let schema = $db.schema();
        
        // Find table by name
        let table = schema.db.tables.iter()
            .find(|t| t.name == $table_name)
            .expect(&format!("Table '{}' not found", $table_name));
        
        let table_id = toasty_core::schema::db::TableId(
            schema.db.tables.iter().position(|t| t.name == $table_name).unwrap()
        );
        
        vec![
            $(
                toasty_core::schema::db::ColumnId {
                    table: table_id,
                    index: table.columns.iter()
                        .position(|c| c.name == $col)
                        .expect(&format!("Column '{}' not found in table '{}'", $col, $table_name))
                }
            ),*
        ]
    }};
}
