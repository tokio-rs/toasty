use toasty_core::{
    driver::Capability,
    schema::{db::Migration, diff},
};

/// Generates D1-compatible SQL for a schema difference.
///
/// The returned statements are intended to be written to a Wrangler migration
/// file and applied by Wrangler rather than through a live D1 binding.
pub fn generate_migration(schema_diff: &diff::Schema<'_>) -> Migration {
    let statements = toasty_sql::MigrationStatement::from_diff(schema_diff, &Capability::D1);
    let sql = statements
        .iter()
        .map(|statement| {
            toasty_sql::Serializer::sqlite(statement.schema()).serialize(statement.statement())
        })
        .collect::<Vec<_>>();

    Migration::new_sql_with_breakpoints(&sql)
}
