/// Helper function to look up TableId by table name (handles database-specific prefixes)
pub fn table(db: &toasty::Db, table_name: &str) -> toasty_core::schema::db::TableId {
    let schema = db.schema();

    // First try exact match
    if let Some(position) = schema.db.tables.iter().position(|t| t.name == table_name) {
        return toasty_core::schema::db::TableId(position);
    }

    // If not found, try to find a table that ends with the given name (for database prefixes)
    if let Some(position) = schema
        .db
        .tables
        .iter()
        .position(|t| t.name.ends_with(table_name))
    {
        return toasty_core::schema::db::TableId(position);
    }

    // If still not found, show available tables for debugging
    let available_tables: Vec<_> = schema.db.tables.iter().map(|t| &t.name).collect();
    panic!(
        "Table '{}' not found. Available tables: {:?}",
        table_name, available_tables
    );
}

/// Helper function to get a single ColumnId for specified table and column
pub fn column(
    db: &toasty::Db,
    table_name: &str,
    column_name: &str,
) -> toasty_core::schema::db::ColumnId {
    columns(db, table_name, &[column_name])[0]
}

/// Helper function to generate a Vec<ColumnId> for specified table and columns
pub fn columns(
    db: &toasty::Db,
    table_name: &str,
    column_names: &[&str],
) -> Vec<toasty_core::schema::db::ColumnId> {
    let schema = db.schema();

    // Find the table using the same logic as table_id (handles prefixes)
    let table_def = schema
        .db
        .tables
        .iter()
        .find(|t| t.name == table_name || t.name.ends_with(table_name))
        .unwrap_or_else(|| panic!("Table '{}' not found", table_name));

    let table_id = table(db, table_name);

    column_names
        .iter()
        .map(|col_name| {
            let index = table_def
                .columns
                .iter()
                .position(|c| c.name == *col_name)
                .unwrap_or_else(|| {
                    panic!("Column '{}' not found in table '{}'", col_name, table_name)
                });

            toasty_core::schema::db::ColumnId {
                table: table_id,
                index,
            }
        })
        .collect()
}

/// Helper function to get an IndexId by table name and column name
pub fn index(
    db: &toasty::Db,
    table_name: &str,
    column_name: &str,
) -> toasty_core::schema::db::IndexId {
    let schema = db.schema();
    let table_id = table(db, table_name);

    // Find the table
    let table_def = schema
        .db
        .tables
        .iter()
        .find(|t| t.name == table_name || t.name.ends_with(table_name))
        .unwrap_or_else(|| panic!("Table '{}' not found", table_name));

    // Find the index by looking for an index on the specified column
    let column_id = column(db, table_name, column_name);

    // Look for an index that contains this column
    let index_position = table_def
        .indices
        .iter()
        .position(|idx| idx.columns.iter().any(|col| col.column == column_id))
        .unwrap_or_else(|| {
            panic!(
                "No index found on column '{}' in table '{}'",
                column_name, table_name
            )
        });

    toasty_core::schema::db::IndexId {
        table: table_id,
        index: index_position,
    }
}
