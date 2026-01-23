/// Database migration generate from a [`super::SchemaDiff`] by a driver.
pub enum Migration {
    Sql(String),
}
