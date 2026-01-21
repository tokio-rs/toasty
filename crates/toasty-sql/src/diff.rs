use toasty_core::{
    driver::Capability,
    schema::db::{SchemaDiff, TablesDiffItem},
};

use crate::stmt::Statement;

impl Statement {
    pub fn from_schema_diff(schema_diff: &SchemaDiff<'_>, capability: &Capability) -> Vec<Self> {
        let mut result = Vec::new();
        for table in schema_diff.tables().iter() {
            match table {
                TablesDiffItem::CreateTable(table) => {
                    result.push(Statement::create_table(table, capability))
                }
                TablesDiffItem::DropTable(table) => result.push(Statement::drop_table(table)),
                TablesDiffItem::AlterTable { from, to, .. } => {
                    result.push(Statement::drop_table(from));
                    result.push(Statement::create_table(to, capability));
                }
            }
        }
        result
    }
}
