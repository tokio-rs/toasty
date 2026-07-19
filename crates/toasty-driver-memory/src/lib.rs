//! Reusable in-memory storage and read execution for Toasty drivers.

use async_trait::async_trait;
use std::{borrow::Cow, cmp::Ordering, sync::Arc};
use toasty_core::{
    Error, Result, Schema,
    driver::{
        Capability, ConnectContext, Driver, ExecResponse, Operation, Rows, SchemaMutations,
        StorageTypes, operation::Pagination,
    },
    schema::{
        db::{self, AppliedMigration, IndexId, Migration, TableId},
        diff,
    },
    stmt::{self, Input, Project, Projection, Value, ValueRecord},
};

/// Capabilities of the read-only in-memory executor.
pub static CAPABILITY: Capability = Capability {
    data_mutations: false,
    sql: false,
    sql_placeholder: None,
    storage_types: StorageTypes {
        default_string_type: db::Type::Text,
        varchar: None,
        default_uuid_type: db::Type::Uuid,
        default_bytes_type: db::Type::Blob,
        default_decimal_type: db::Type::Numeric(None),
        default_bigdecimal_type: db::Type::Numeric(None),
        default_timestamp_type: db::Type::Timestamp(9),
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Date,
        default_time_type: db::Type::Time(9),
        default_datetime_type: db::Type::DateTime(9),
        max_unsigned_integer: None,
    },
    schema_mutations: SchemaMutations {
        alter_column_type: false,
        alter_column_properties_atomic: false,
    },
    primary_key_ne_predicate: true,
    bool_key_type: true,
    native_timestamp: true,
    native_date: true,
    native_time: true,
    native_datetime: true,
    native_decimal: true,
    decimal_arbitrary_precision: true,
    bigdecimal_implemented: true,
    index_or_predicate: true,
    native_starts_with: true,
    native_like: true,
    native_ilike: true,
    scan_supports_sort: true,
    backward_pagination: true,
    native_array: true,
    bind_list_param: true,
    predicate_match_any: true,
    native_array_set_predicates: true,
    ..Capability::DYNAMODB
};

/// Read access needed by [`Reader`].
pub trait ReadStore: std::fmt::Debug + Send + Sync + 'static {
    /// Returns every row in a table in storage order.
    fn rows(&self, table: TableId) -> &[ValueRecord];

    /// Returns row offsets in the natural order of an index.
    fn index_rows(&self, table: TableId, index: IndexId) -> Option<Vec<usize>>;
}

#[derive(Debug)]
struct IndexSnapshot {
    rows: Vec<usize>,
}

#[derive(Debug)]
struct TableSnapshot {
    rows: Vec<ValueRecord>,
    indices: Vec<IndexSnapshot>,
}

/// An immutable collection of typed database rows and index orderings.
#[derive(Debug)]
pub struct Snapshot {
    tables: Vec<TableSnapshot>,
}

impl ReadStore for Snapshot {
    fn rows(&self, table: TableId) -> &[ValueRecord] {
        &self.tables[table.0].rows
    }

    fn index_rows(&self, table: TableId, index: IndexId) -> Option<Vec<usize>> {
        self.tables
            .get(table.0)?
            .indices
            .get(index.index)
            .map(|index| index.rows.clone())
    }
}

/// Builds and validates an immutable [`Snapshot`].
#[derive(Debug)]
pub struct SnapshotBuilder {
    schema: Arc<Schema>,
    tables: Vec<Vec<ValueRecord>>,
}

impl SnapshotBuilder {
    /// Creates an empty builder for a compiled schema.
    pub fn new(schema: Arc<Schema>) -> Self {
        Self {
            tables: vec![Vec::new(); schema.db.tables.len()],
            schema,
        }
    }

    /// Adds one row in database-column order.
    pub fn insert(&mut self, table: TableId, row: ValueRecord) -> Result<()> {
        let Some(rows) = self.tables.get_mut(table.0) else {
            return Err(Error::invalid_schema(format!(
                "invalid table ID {} while building memory snapshot",
                table.0
            )));
        };
        rows.push(row);
        Ok(())
    }

    /// Adds rows to the table with the given database name.
    pub fn insert_named(
        &mut self,
        table_name: &str,
        rows: impl IntoIterator<Item = ValueRecord>,
    ) -> Result<()> {
        let table = self
            .schema
            .db
            .tables
            .iter()
            .find(|table| table.name == table_name)
            .ok_or_else(|| {
                Error::invalid_schema(format!("unknown database table `{table_name}`"))
            })?;
        self.tables[table.id.0].extend(rows);
        Ok(())
    }

    /// Validates all rows and builds declared index orderings.
    pub fn build(self) -> Result<Snapshot> {
        let mut tables = Vec::with_capacity(self.tables.len());

        for (table_index, rows) in self.tables.into_iter().enumerate() {
            let table = &self.schema.db.tables[table_index];
            for (row_index, row) in rows.iter().enumerate() {
                validate_row(&self.schema, table, row_index, row)?;
            }

            let mut indices = Vec::with_capacity(table.indices.len());
            for index in &table.indices {
                let mut entries: Vec<(Vec<Value>, usize)> = rows
                    .iter()
                    .enumerate()
                    .map(|(row, value)| (index_key(value, index), row))
                    .collect();

                if index.unique || index.primary_key {
                    for left in 0..entries.len() {
                        if entries[left].0.iter().any(Value::is_null) {
                            continue;
                        }
                        if entries[left + 1..]
                            .iter()
                            .any(|entry| entry.0 == entries[left].0)
                        {
                            return Err(Error::invalid_schema(format!(
                                "duplicate value for index `{}` on table `{}`",
                                index.name, table.name
                            )));
                        }
                    }
                }

                entries.sort_by(|left, right| compare_keys(&left.0, &right.0));
                indices.push(IndexSnapshot {
                    rows: entries.into_iter().map(|entry| entry.1).collect(),
                });
            }

            tables.push(TableSnapshot { rows, indices });
        }

        Ok(Snapshot { tables })
    }
}

fn validate_row(
    schema: &Schema,
    table: &db::Table,
    row_index: usize,
    row: &ValueRecord,
) -> Result<()> {
    if row.len() != table.columns.len() {
        return Err(Error::invalid_schema(format!(
            "row {row_index} in table `{}` has {} values; expected {}",
            table.name,
            row.len(),
            table.columns.len()
        )));
    }

    for (column, value) in table.columns.iter().zip(row.iter()) {
        if value.is_null() && (!column.nullable || column.primary_key) {
            return Err(Error::invalid_schema(format!(
                "row {row_index} in table `{}` has null for required column `{}`",
                table.name, column.name
            )));
        }
        if !value.is_a(schema, &column.ty) {
            return Err(Error::invalid_schema(format!(
                "row {row_index} in table `{}` has an invalid value for column `{}`; expected {:?}, got {:?}",
                table.name, column.name, column.ty, value
            )));
        }
    }
    Ok(())
}

fn index_key(row: &ValueRecord, index: &db::Index) -> Vec<Value> {
    index
        .columns
        .iter()
        .map(|column| row[column.column.index].clone())
        .collect()
}

fn primary_key(row: &ValueRecord, table: &db::Table) -> Vec<Value> {
    table
        .primary_key
        .columns
        .iter()
        .map(|column| row[column.index].clone())
        .collect()
}

fn primary_key_value(row: &ValueRecord, table: &db::Table) -> Value {
    Value::record_from_vec(primary_key(row, table))
}

fn key_matches(row: &ValueRecord, table: &db::Table, key: &Value) -> bool {
    let actual = primary_key(row, table);
    match key {
        Value::Record(key) => actual.as_slice() == key.as_slice(),
        key => actual.len() == 1 && actual[0] == *key,
    }
}

fn compare_keys(left: &[Value], right: &[Value]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let order = compare_values(left, right, stmt::Direction::Asc);
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_values(left: &Value, right: &Value, direction: stmt::Direction) -> Ordering {
    let order = match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        _ => left.partial_cmp(right).unwrap_or(Ordering::Equal),
    };
    match direction {
        stmt::Direction::Asc => order,
        stmt::Direction::Desc => order.reverse(),
    }
}

/// Executes Toasty read operations against a [`ReadStore`].
#[derive(Debug, Clone)]
pub struct Reader {
    store: Arc<dyn ReadStore>,
}

impl Reader {
    /// Creates a reader for a shared store.
    pub fn new(store: Arc<dyn ReadStore>) -> Self {
        Self { store }
    }

    /// Executes one read or transaction operation.
    pub fn exec(&self, schema: &Arc<Schema>, operation: Operation) -> Result<ExecResponse> {
        match operation {
            Operation::GetByKey(operation) => self.get_by_key(schema, operation),
            Operation::FindPkByIndex(operation) => self.find_pk_by_index(schema, operation),
            Operation::QueryPk(operation) => self.query_pk(schema, operation),
            Operation::Scan(operation) => self.scan(schema, operation),
            Operation::Transaction(_) => Ok(ExecResponse::count(0)),
            Operation::Insert(_) | Operation::DeleteByKey(_) | Operation::UpdateByKey(_) => {
                Err(read_only_error())
            }
            Operation::QuerySql(_) | Operation::RawSql(_) => Err(Error::unsupported_feature(
                "SQL operations are not supported by the in-memory reader",
            )),
        }
    }

    fn get_by_key(
        &self,
        schema: &Arc<Schema>,
        operation: toasty_core::driver::operation::GetByKey,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(operation.table);
        let rows = self.store.rows(operation.table);
        let mut result = Vec::new();
        for key in &operation.keys {
            if let Some(row) = rows.iter().find(|row| key_matches(row, table, key)) {
                result.push(project(
                    row,
                    operation.select.iter().map(|column| column.index),
                ));
            }
        }
        Ok(response(result, None, None))
    }

    fn find_pk_by_index(
        &self,
        schema: &Arc<Schema>,
        operation: toasty_core::driver::operation::FindPkByIndex,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(operation.table);
        let rows = self.store.rows(operation.table);
        let candidates = self
            .store
            .index_rows(operation.table, operation.index)
            .unwrap_or_else(|| (0..rows.len()).collect());
        let mut result = Vec::new();
        for row in candidates {
            if eval_predicate(schema, &rows[row], &operation.filter)? {
                result.push(Value::record_from_vec(primary_key(&rows[row], table)));
            }
        }
        Ok(response(result, None, None))
    }

    fn query_pk(
        &self,
        schema: &Arc<Schema>,
        operation: toasty_core::driver::operation::QueryPk,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(operation.table);
        let index = operation.index.unwrap_or(table.primary_key.index);
        let rows = self.store.rows(operation.table);
        let mut matches = self
            .store
            .index_rows(operation.table, index)
            .unwrap_or_else(|| (0..rows.len()).collect());
        if matches!(operation.order, Some(stmt::Direction::Desc)) {
            matches.reverse();
        }
        let mut filtered = Vec::with_capacity(matches.len());
        for row in matches {
            if !eval_predicate(schema, &rows[row], &operation.pk_filter)? {
                continue;
            }
            if let Some(filter) = &operation.filter
                && !eval_predicate(schema, &rows[row], filter)?
            {
                continue;
            }
            filtered.push(row);
        }
        let matches = filtered;

        let (matches, next_cursor, prev_cursor) = paginate(matches, operation.limit, rows, table)?;
        let result = matches
            .into_iter()
            .map(|row| {
                project(
                    &rows[row],
                    operation.select.iter().map(|column| column.index),
                )
            })
            .collect();
        Ok(response(result, next_cursor, prev_cursor))
    }

    fn scan(
        &self,
        schema: &Arc<Schema>,
        operation: toasty_core::driver::operation::Scan,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(operation.table);
        let rows = self.store.rows(operation.table);
        let mut matches: Vec<usize> = (0..rows.len()).collect();

        if let Some(filter) = &operation.filter {
            let mut filtered = Vec::with_capacity(matches.len());
            for row in matches {
                if eval_predicate(schema, &rows[row], filter)? {
                    filtered.push(row);
                }
            }
            matches = filtered;
        }

        if let Some(order_by) = &operation.order_by {
            let mut keys = Vec::with_capacity(matches.len());
            for row in matches {
                let input = RowInput {
                    schema,
                    row: &rows[row],
                };
                let values = order_by
                    .exprs
                    .iter()
                    .map(|order| order.expr.eval(input.clone()))
                    .collect::<Result<Vec<_>>>()?;
                keys.push((row, values));
            }
            keys.sort_by(|left, right| {
                for (value_index, order) in order_by.exprs.iter().enumerate() {
                    let direction = order.order.unwrap_or(stmt::Direction::Asc);
                    let ordering =
                        compare_values(&left.1[value_index], &right.1[value_index], direction);
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                }
                compare_keys(
                    &primary_key(&rows[left.0], table),
                    &primary_key(&rows[right.0], table),
                )
            });
            matches = keys.into_iter().map(|entry| entry.0).collect();
        } else {
            matches.sort_by(|left, right| {
                compare_keys(
                    &primary_key(&rows[*left], table),
                    &primary_key(&rows[*right], table),
                )
            });
        }

        let (matches, next_cursor, prev_cursor) = paginate(matches, operation.limit, rows, table)?;
        let result = matches
            .into_iter()
            .map(|row| project(&rows[row], operation.columns.iter().copied()))
            .collect();
        Ok(response(result, next_cursor, prev_cursor))
    }
}

fn paginate(
    rows: Vec<usize>,
    pagination: Option<Pagination>,
    values: &[ValueRecord],
    table: &db::Table,
) -> Result<(Vec<usize>, Option<Value>, Option<Value>)> {
    let Some(pagination) = pagination else {
        return Ok((rows, None, None));
    };

    match pagination {
        Pagination::Offset { limit, offset } => {
            let limit = usize::try_from(limit)
                .map_err(|_| Error::invalid_statement("memory query limit must be non-negative"))?;
            let offset = usize::try_from(offset.unwrap_or(0)).map_err(|_| {
                Error::invalid_statement("memory query offset must be non-negative")
            })?;
            Ok((
                rows.into_iter().skip(offset).take(limit).collect(),
                None,
                None,
            ))
        }
        Pagination::Cursor { page_size, after } => {
            let page_size = usize::try_from(page_size)
                .map_err(|_| Error::invalid_statement("memory page size must be non-negative"))?;
            let start = match after {
                Some(after) => rows
                    .iter()
                    .position(|row| key_matches(&values[*row], table, &after))
                    .map(|index| index + 1)
                    .ok_or_else(|| {
                        Error::invalid_statement("cursor does not belong to this query")
                    })?,
                None => 0,
            };
            let end = start.saturating_add(page_size).min(rows.len());
            let page = rows[start..end].to_vec();
            let next_cursor = if end < rows.len() {
                page.last()
                    .map(|row| primary_key_value(&values[*row], table))
            } else {
                None
            };
            let prev_cursor = if start > 0 {
                page.first()
                    .map(|row| primary_key_value(&values[*row], table))
            } else {
                None
            };
            Ok((page, next_cursor, prev_cursor))
        }
    }
}

fn project(row: &ValueRecord, columns: impl Iterator<Item = usize>) -> Value {
    Value::record_from_vec(columns.map(|column| row[column].clone()).collect())
}

fn response(
    rows: Vec<Value>,
    next_cursor: Option<Value>,
    prev_cursor: Option<Value>,
) -> ExecResponse {
    ExecResponse {
        values: Rows::Stream(stmt::ValueStream::from_vec(rows)),
        next_cursor,
        prev_cursor,
    }
}

#[derive(Clone)]
struct RowInput<'a> {
    schema: &'a Schema,
    row: &'a ValueRecord,
}

impl Input for RowInput<'_> {
    fn resolve_ref(
        &mut self,
        reference: &stmt::ExprReference,
        projection: &Projection,
    ) -> Option<stmt::Expr> {
        let value = match reference {
            stmt::ExprReference::Column(column) if column.nesting == 0 && column.table == 0 => {
                self.row.get(column.column)?
            }
            stmt::ExprReference::Field { nesting: 0, index } => self.row.get(*index)?,
            stmt::ExprReference::Model { nesting: 0 } => {
                return Value::Record(self.row.clone()).project(projection);
            }
            _ => return None,
        };
        value.project(projection)
    }

    fn resolve_model(
        &self,
        id: toasty_core::schema::app::ModelId,
    ) -> Option<&toasty_core::schema::app::Model> {
        Some(self.schema.app.model(id))
    }

    fn ordered_nulls_are_false(&self) -> bool {
        true
    }
}

fn eval_predicate(schema: &Schema, row: &ValueRecord, expression: &stmt::Expr) -> Result<bool> {
    expression.eval_bool(RowInput { schema, row })
}

fn read_only_error() -> Error {
    Error::unsupported_feature("the in-memory store is read-only")
}

/// A schema-initialized connection backed by a [`ReadStore`].
#[derive(Debug)]
pub struct Connection {
    reader: Reader,
}

impl Connection {
    /// Creates a connection over a shared store.
    pub fn new(store: Arc<dyn ReadStore>) -> Self {
        Self {
            reader: Reader::new(store),
        }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, operation: Operation) -> Result<ExecResponse> {
        self.reader.exec(schema, operation)
    }

    async fn push_schema(&mut self, _schema: &Schema) -> Result<()> {
        Err(read_only_error())
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        Err(read_only_error())
    }

    async fn apply_migration(
        &mut self,
        _id: u64,
        _name: &str,
        _migration: &Migration,
    ) -> Result<()> {
        Err(read_only_error())
    }
}

/// Builder for a read-only in-memory [`Driver`].
#[derive(Debug, Default)]
pub struct MemoryBuilder {
    tables: Vec<(String, Vec<ValueRecord>)>,
}

impl MemoryBuilder {
    /// Supplies all rows for one database table.
    pub fn table(
        mut self,
        name: impl Into<String>,
        rows: impl IntoIterator<Item = ValueRecord>,
    ) -> Self {
        self.tables.push((name.into(), rows.into_iter().collect()));
        self
    }

    /// Builds a driver whose rows are validated during `Db::build()`.
    pub fn build(self) -> Memory {
        Memory {
            source: self.tables,
            snapshot: None,
        }
    }
}

/// A read-only Toasty driver backed by rows supplied in memory.
#[derive(Debug)]
pub struct Memory {
    source: Vec<(String, Vec<ValueRecord>)>,
    snapshot: Option<Arc<Snapshot>>,
}

impl Memory {
    /// Creates a builder for an in-memory driver.
    pub fn builder() -> MemoryBuilder {
        MemoryBuilder::default()
    }
}

#[async_trait]
impl Driver for Memory {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed("memory://")
    }

    fn capability(&self) -> &'static Capability {
        &CAPABILITY
    }

    async fn initialize(&mut self, schema: &Arc<Schema>) -> Result<()> {
        let mut builder = SnapshotBuilder::new(schema.clone());
        for (name, rows) in std::mem::take(&mut self.source) {
            builder.insert_named(&name, rows)?;
        }
        self.snapshot = Some(Arc::new(builder.build()?));
        Ok(())
    }

    async fn connect(&self, _cx: &ConnectContext) -> Result<Box<dyn toasty_core::Connection>> {
        let snapshot = self.snapshot.clone().ok_or_else(|| {
            Error::invalid_driver_configuration("memory driver was not initialized")
        })?;
        Ok(Box::new(Connection::new(snapshot)))
    }

    fn max_connections(&self) -> Option<usize> {
        None
    }

    fn generate_migration(&self, _schema_diff: &diff::Schema<'_>) -> Migration {
        Migration::new_sql(String::new())
    }

    async fn reset_db(&self) -> Result<()> {
        Err(read_only_error())
    }
}
