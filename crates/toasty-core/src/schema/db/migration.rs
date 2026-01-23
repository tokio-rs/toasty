/// Database migration generate from a [`super::SchemaDiff`] by a driver.
pub enum Migration {
    Sql { statements: Vec<String> },
}
