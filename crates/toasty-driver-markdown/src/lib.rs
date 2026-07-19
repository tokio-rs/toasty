//! Read-only Toasty driver for Markdown files with YAML front matter.

use async_trait::async_trait;
use serde_yaml::{Mapping, Value as Yaml};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    Error, Result, Schema,
    driver::{Capability, ConnectContext, Driver},
    schema::{db::Migration, diff},
    stmt::{self, Value, ValueObject, ValueRecord},
};
use toasty_driver_memory::{Connection, Snapshot, SnapshotBuilder};

/// Selects how a file path populates a string column.
#[derive(Debug, Clone)]
enum PathKey {
    Stem(String),
    RelativePath(String),
}

/// Per-table Markdown mapping configuration.
#[derive(Debug, Clone)]
pub struct Table {
    directory: PathBuf,
    columns: HashMap<String, String>,
    body_column: BodyColumn,
    path_key: Option<PathKey>,
    recursive: bool,
}

#[derive(Debug, Clone, Default)]
enum BodyColumn {
    #[default]
    Conventional,
    Named(String),
    Disabled,
}

impl Table {
    /// Creates a table mapping rooted at `directory`, relative to the content root.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            columns: HashMap::new(),
            body_column: BodyColumn::Conventional,
            path_key: None,
            recursive: false,
        }
    }

    /// Maps a YAML front-matter key to a database column name.
    pub fn column(mut self, front_matter: impl Into<String>, column: impl Into<String>) -> Self {
        self.columns.insert(front_matter.into(), column.into());
        self
    }

    /// Maps the Markdown body to the named string column.
    pub fn body_column(mut self, column: impl Into<String>) -> Self {
        self.body_column = BodyColumn::Named(column.into());
        self
    }

    /// Disables Markdown body mapping for this table.
    pub fn disable_body(mut self) -> Self {
        self.body_column = BodyColumn::Disabled;
        self
    }

    /// Populates the named string column from each file stem.
    pub fn key_from_stem(mut self, column: impl Into<String>) -> Self {
        self.path_key = Some(PathKey::Stem(column.into()));
        self
    }

    /// Populates the named string column from the relative path without `.md`.
    pub fn key_from_relative_path(mut self, column: impl Into<String>) -> Self {
        self.path_key = Some(PathKey::RelativePath(column.into()));
        self
    }

    /// Enables or disables recursive `.md` discovery.
    pub fn recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }
}

/// Builder for a [`Markdown`] driver.
#[derive(Debug)]
pub struct Builder {
    root: PathBuf,
    tables: HashMap<String, Table>,
    strict: bool,
}

impl Builder {
    /// Overrides conventions for one database table.
    pub fn table(mut self, table: impl Into<String>, config: Table) -> Self {
        self.tables.insert(table.into(), config);
        self
    }

    /// Enables errors for unknown directories, metadata, and unmapped bodies.
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Builds the driver. Files are loaded later during `Db::build()`.
    pub fn build(self) -> Markdown {
        Markdown {
            root: self.root,
            tables: self.tables,
            strict: self.strict,
            snapshot: None,
        }
    }
}

/// A read-only driver that presents Markdown files as Toasty rows.
#[derive(Debug)]
pub struct Markdown {
    root: PathBuf,
    tables: HashMap<String, Table>,
    strict: bool,
    snapshot: Option<Arc<Snapshot>>,
}

impl Markdown {
    /// Creates a driver using the default directory and column conventions.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::builder(root).build()
    }

    /// Creates a configurable Markdown driver builder.
    pub fn builder(root: impl Into<PathBuf>) -> Builder {
        Builder {
            root: root.into(),
            tables: HashMap::new(),
            strict: false,
        }
    }

    fn load(&self, schema: &Arc<Schema>) -> Result<Snapshot> {
        let root = self.root.canonicalize().map_err(|error| {
            Error::from_args(format_args!(
                "failed to open Markdown root `{}`: {error}",
                self.root.display()
            ))
        })?;
        if !root.is_dir() {
            return Err(Error::invalid_driver_configuration(format!(
                "Markdown root `{}` is not a directory",
                root.display()
            )));
        }

        for table_name in self.tables.keys() {
            if !schema
                .db
                .tables
                .iter()
                .any(|table| &table.name == table_name)
            {
                return Err(Error::invalid_driver_configuration(format!(
                    "Markdown configuration names unknown table `{table_name}`"
                )));
            }
        }

        if self.strict {
            self.validate_root_directories(schema, &root)?;
        }

        let mut snapshot = SnapshotBuilder::new(schema.clone());
        for table in &schema.db.tables {
            let explicit = self.tables.get(&table.name);
            let config = explicit
                .cloned()
                .unwrap_or_else(|| Table::new(table.name.clone()));
            validate_table_config(table, &config)?;
            validate_relative_directory(&config.directory)?;
            let directory = root.join(&config.directory);

            if !directory.exists() {
                if explicit.is_some() {
                    return Err(Error::invalid_driver_configuration(format!(
                        "configured Markdown directory `{}` does not exist",
                        directory.display()
                    )));
                }
                continue;
            }

            let metadata = fs::symlink_metadata(&directory).map_err(file_error(&directory))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(Error::invalid_driver_configuration(format!(
                    "Markdown table path `{}` must be a real directory",
                    directory.display()
                )));
            }

            let canonical = directory.canonicalize().map_err(file_error(&directory))?;
            if !canonical.starts_with(&root) {
                return Err(Error::invalid_driver_configuration(format!(
                    "Markdown table path `{}` escapes the content root",
                    directory.display()
                )));
            }

            let mut files = Vec::new();
            discover_files(&directory, config.recursive, self.strict, &mut files)?;
            files.sort();
            for path in files {
                let row = self.load_file(schema, table, &config, &directory, &path)?;
                snapshot.insert(table.id, row)?;
            }
        }
        snapshot.build()
    }

    fn validate_root_directories(&self, schema: &Schema, root: &Path) -> Result<()> {
        let expected: HashSet<PathBuf> = schema
            .db
            .tables
            .iter()
            .map(|table| {
                self.tables
                    .get(&table.name)
                    .map(|config| config.directory.clone())
                    .unwrap_or_else(|| PathBuf::from(&table.name))
            })
            .filter_map(|path| {
                path.components()
                    .next()
                    .map(|component| PathBuf::from(component.as_os_str()))
            })
            .collect();

        for entry in fs::read_dir(root).map_err(file_error(root))? {
            let entry = entry.map_err(file_error(root))?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(file_error(&entry.path()))?;
            if metadata.is_dir() && !expected.contains(&PathBuf::from(entry.file_name())) {
                return Err(Error::invalid_driver_configuration(format!(
                    "unknown directory `{}` in strict Markdown root",
                    entry.path().display()
                )));
            }
        }
        Ok(())
    }

    fn load_file(
        &self,
        schema: &Schema,
        table: &toasty_core::schema::db::Table,
        config: &Table,
        directory: &Path,
        path: &Path,
    ) -> Result<ValueRecord> {
        let source = fs::read_to_string(path).map_err(file_error(path))?;
        let (front_matter, body) = split_document(path, &source)?;
        let mapping = parse_front_matter(path, front_matter)?;
        let mut values = vec![Value::Null; table.columns.len()];
        let mut populated = HashSet::new();

        for (key, yaml) in mapping {
            let Some(key) = key.as_str() else {
                return Err(Error::invalid_driver_configuration(format!(
                    "front matter in `{}` contains a non-string key",
                    path.display()
                )));
            };
            let column_name = config.columns.get(key).map(String::as_str).unwrap_or(key);
            let Some(column) = table
                .columns
                .iter()
                .find(|column| column.name == column_name)
            else {
                if self.strict {
                    return Err(Error::invalid_driver_configuration(format!(
                        "unknown front-matter key `{key}` in `{}`",
                        path.display()
                    )));
                }
                continue;
            };
            if !populated.insert(column.id.index) {
                return Err(Error::invalid_driver_configuration(format!(
                    "more than one source populates column `{}` in `{}`",
                    column.name,
                    path.display()
                )));
            }
            values[column.id.index] = decode_yaml(schema, &column.ty, yaml).map_err(|error| {
                error.context(Error::from_args(format_args!(
                    "invalid front-matter key `{key}` in `{}`",
                    path.display()
                )))
            })?;
        }

        let configured_path_key = config.path_key.is_some();
        let path_key = config.path_key.clone().or_else(|| {
            (table.primary_key.columns.len() == 1)
                .then(|| table.primary_key_column(0))
                .filter(|column| column.ty == stmt::Type::String)
                .map(|column| PathKey::Stem(column.name.clone()))
        });
        if let Some(path_key) = path_key {
            let (column_name, value) = match path_key {
                PathKey::Stem(column) => (
                    column,
                    path.file_stem()
                        .and_then(OsStr::to_str)
                        .map(str::to_owned)
                        .ok_or_else(|| invalid_utf8_path(path))?,
                ),
                PathKey::RelativePath(column) => {
                    let relative = path.strip_prefix(directory).map_err(|_| {
                        Error::invalid_driver_configuration("Markdown path escaped its table")
                    })?;
                    let mut relative = relative.to_path_buf();
                    relative.set_extension("");
                    let value = relative
                        .components()
                        .map(|component| component.as_os_str().to_str())
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| invalid_utf8_path(path))?
                        .join("/");
                    (column, value)
                }
            };
            let column = table
                .columns
                .iter()
                .find(|column| column.name == column_name)
                .ok_or_else(|| {
                    Error::invalid_driver_configuration(format!(
                        "path key names unknown column `{column_name}` on table `{}`",
                        table.name
                    ))
                })?;
            if column.ty != stmt::Type::String {
                return Err(Error::invalid_driver_configuration(format!(
                    "path-derived column `{}` on table `{}` must be String",
                    column.name, table.name
                )));
            }
            if populated.contains(&column.id.index) {
                if configured_path_key {
                    return Err(Error::invalid_driver_configuration(format!(
                        "front matter and the path both populate column `{}` in `{}`",
                        column.name,
                        path.display()
                    )));
                }
            } else {
                values[column.id.index] = Value::String(value);
                populated.insert(column.id.index);
            }
        }

        let body_column = match &config.body_column {
            BodyColumn::Conventional => table
                .columns
                .iter()
                .find(|column| column.name == "body")
                .map(|column| column.name.as_str()),
            BodyColumn::Named(column) => Some(column.as_str()),
            BodyColumn::Disabled => None,
        };
        if let Some(body_column) = body_column {
            let column = table
                .columns
                .iter()
                .find(|column| column.name == body_column)
                .ok_or_else(|| {
                    Error::invalid_driver_configuration(format!(
                        "body mapping names unknown column `{body_column}` on table `{}`",
                        table.name
                    ))
                })?;
            if column.ty != stmt::Type::String {
                return Err(Error::invalid_driver_configuration(format!(
                    "body column `{body_column}` on table `{}` must be String",
                    table.name
                )));
            }
            if populated.contains(&column.id.index) {
                return Err(Error::invalid_driver_configuration(format!(
                    "front matter and the Markdown body both populate column `{body_column}` in `{}`",
                    path.display()
                )));
            }
            values[column.id.index] = Value::String(body.to_owned());
            populated.insert(column.id.index);
        } else if self.strict && !body.is_empty() {
            return Err(Error::invalid_driver_configuration(format!(
                "non-empty Markdown body in `{}` has no body column",
                path.display()
            )));
        }

        for column in &table.columns {
            if values[column.id.index].is_null() && !column.nullable {
                return Err(Error::invalid_schema(format!(
                    "missing required column `{}` in `{}`",
                    column.name,
                    path.display()
                )));
            }
        }

        Ok(ValueRecord::from_vec(values))
    }
}

fn validate_table_config(table: &toasty_core::schema::db::Table, config: &Table) -> Result<()> {
    let mut destinations = HashSet::new();
    for (front_matter, column_name) in &config.columns {
        if !destinations.insert(column_name) {
            return Err(Error::invalid_driver_configuration(format!(
                "front-matter keys map to column `{column_name}` more than once on table `{}`",
                table.name
            )));
        }
        if !table
            .columns
            .iter()
            .any(|column| column.name == *column_name)
        {
            return Err(Error::invalid_driver_configuration(format!(
                "front-matter key `{front_matter}` maps to unknown column `{column_name}` on table `{}`",
                table.name
            )));
        }
    }

    let body_column = match &config.body_column {
        BodyColumn::Conventional => table.columns.iter().find(|column| column.name == "body"),
        BodyColumn::Named(column_name) => Some(
            table
                .columns
                .iter()
                .find(|column| column.name == *column_name)
                .ok_or_else(|| {
                    Error::invalid_driver_configuration(format!(
                        "body mapping names unknown column `{column_name}` on table `{}`",
                        table.name
                    ))
                })?,
        ),
        BodyColumn::Disabled => None,
    };
    if let Some(column) = body_column
        && column.ty != stmt::Type::String
    {
        return Err(Error::invalid_driver_configuration(format!(
            "body column `{}` on table `{}` must be String",
            column.name, table.name
        )));
    }

    if let Some(path_key) = &config.path_key {
        let column_name = match path_key {
            PathKey::Stem(column) | PathKey::RelativePath(column) => column,
        };
        let column = table
            .columns
            .iter()
            .find(|column| column.name == *column_name)
            .ok_or_else(|| {
                Error::invalid_driver_configuration(format!(
                    "path key names unknown column `{column_name}` on table `{}`",
                    table.name
                ))
            })?;
        if column.ty != stmt::Type::String {
            return Err(Error::invalid_driver_configuration(format!(
                "path-derived column `{}` on table `{}` must be String",
                column.name, table.name
            )));
        }
    }

    Ok(())
}

fn validate_relative_directory(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::invalid_driver_configuration(format!(
            "Markdown table directory `{}` must be a relative path inside the root",
            path.display()
        )));
    }
    Ok(())
}

fn discover_files(
    directory: &Path,
    recursive: bool,
    strict: bool,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(file_error(directory))? {
        let entry = entry.map_err(file_error(directory))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(file_error(&path))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if recursive {
                discover_files(&path, true, strict, files)?;
            } else if strict {
                return Err(Error::invalid_driver_configuration(format!(
                    "nested directory `{}` requires recursive discovery",
                    path.display()
                )));
            }
        } else if metadata.is_file() && path.extension() == Some(OsStr::new("md")) {
            files.push(path);
        }
    }
    Ok(())
}

fn split_document<'a>(path: &Path, source: &'a str) -> Result<(Option<&'a str>, &'a str)> {
    let Some(first_end) = source.find('\n') else {
        return if source.trim_end_matches('\r') == "---" {
            Err(Error::invalid_driver_configuration(format!(
                "front matter in `{}` has no closing delimiter",
                path.display()
            )))
        } else {
            Ok((None, source))
        };
    };
    if source[..first_end].trim_end_matches('\r') != "---" {
        return Ok((None, source));
    }

    let content_start = first_end + 1;
    let mut offset = content_start;
    for line in source[content_start..].split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if line_without_newline == "---" {
            let front_matter = &source[content_start..offset];
            let body_start = offset + line.len();
            return Ok((Some(front_matter), &source[body_start..]));
        }
        offset += line.len();
    }
    if offset < source.len() && source[offset..].trim_end_matches('\r') == "---" {
        return Ok((Some(&source[content_start..offset]), ""));
    }

    Err(Error::invalid_driver_configuration(format!(
        "front matter in `{}` has no closing delimiter",
        path.display()
    )))
}

fn parse_front_matter(path: &Path, source: Option<&str>) -> Result<Mapping> {
    let Some(source) = source else {
        return Ok(Mapping::new());
    };
    if source.trim().is_empty() {
        return Ok(Mapping::new());
    }
    match serde_yaml::from_str(source).map_err(|error| {
        Error::invalid_driver_configuration(format!(
            "invalid YAML front matter in `{}`: {error}",
            path.display()
        ))
    })? {
        Yaml::Mapping(mapping) => Ok(mapping),
        Yaml::Null => Ok(Mapping::new()),
        _ => Err(Error::invalid_driver_configuration(format!(
            "front matter in `{}` must be a YAML mapping",
            path.display()
        ))),
    }
}

fn decode_yaml(schema: &Schema, ty: &stmt::Type, yaml: Yaml) -> Result<Value> {
    if matches!(yaml, Yaml::Null) {
        return Ok(Value::Null);
    }

    match ty {
        stmt::Type::List(element) => {
            let Yaml::Sequence(items) = yaml else {
                return type_error(ty, yaml);
            };
            Ok(Value::List(
                items
                    .into_iter()
                    .map(|item| decode_yaml(schema, element, item))
                    .collect::<Result<_>>()?,
            ))
        }
        stmt::Type::Record(fields) => {
            let Yaml::Sequence(items) = yaml else {
                return type_error(ty, yaml);
            };
            if items.len() != fields.len() {
                return Err(Error::invalid_schema(format!(
                    "expected {} record fields, got {}",
                    fields.len(),
                    items.len()
                )));
            }
            Ok(Value::record_from_vec(
                fields
                    .iter()
                    .zip(items)
                    .map(|(field, item)| decode_yaml(schema, field, item))
                    .collect::<Result<_>>()?,
            ))
        }
        stmt::Type::Object => yaml_to_object(yaml),
        stmt::Type::Bytes => match yaml {
            Yaml::Sequence(items) => Ok(Value::Bytes(
                items
                    .into_iter()
                    .map(|item| match decode_yaml(schema, &stmt::Type::U8, item)? {
                        Value::U8(value) => Ok(value),
                        _ => unreachable!(),
                    })
                    .collect::<Result<_>>()?,
            )),
            yaml => type_error(ty, yaml),
        },
        stmt::Type::Uuid => match yaml {
            Yaml::String(value) => value
                .parse()
                .map(Value::Uuid)
                .map_err(|_| Error::invalid_schema(format!("invalid UUID value `{value}`"))),
            yaml => type_error(ty, yaml),
        },
        #[cfg(feature = "rust_decimal")]
        stmt::Type::Decimal => match yaml {
            Yaml::String(value) => value
                .parse()
                .map(Value::Decimal)
                .map_err(|_| Error::invalid_schema(format!("invalid Decimal value `{value}`"))),
            Yaml::Number(value) => value
                .to_string()
                .parse()
                .map(Value::Decimal)
                .map_err(|_| Error::invalid_schema(format!("invalid Decimal value `{value}`"))),
            yaml => type_error(ty, yaml),
        },
        #[cfg(feature = "bigdecimal")]
        stmt::Type::BigDecimal => match yaml {
            Yaml::String(value) => value
                .parse()
                .map(Value::BigDecimal)
                .map_err(|_| Error::invalid_schema(format!("invalid BigDecimal value `{value}`"))),
            Yaml::Number(value) => value
                .to_string()
                .parse()
                .map(Value::BigDecimal)
                .map_err(|_| Error::invalid_schema(format!("invalid BigDecimal value `{value}`"))),
            yaml => type_error(ty, yaml),
        },
        #[cfg(feature = "jiff")]
        stmt::Type::Timestamp => parse_string_value(yaml, ty, Value::Timestamp),
        #[cfg(feature = "jiff")]
        stmt::Type::Zoned => parse_string_value(yaml, ty, Value::Zoned),
        #[cfg(feature = "jiff")]
        stmt::Type::Date => parse_string_value(yaml, ty, Value::Date),
        #[cfg(feature = "jiff")]
        stmt::Type::Time => parse_string_value(yaml, ty, Value::Time),
        #[cfg(feature = "jiff")]
        stmt::Type::DateTime => parse_string_value(yaml, ty, Value::DateTime),
        _ => {
            let value = yaml_to_value(yaml)?;
            if value.is_a(schema, ty) {
                Ok(value)
            } else {
                ty.cast(schema, value)
            }
        }
    }
}

#[cfg(feature = "jiff")]
fn parse_string_value<T>(
    yaml: Yaml,
    ty: &stmt::Type,
    constructor: impl FnOnce(T) -> Value,
) -> Result<Value>
where
    T: std::str::FromStr,
{
    let Yaml::String(value) = yaml else {
        return type_error(ty, yaml);
    };
    value
        .parse()
        .map(constructor)
        .map_err(|_| Error::invalid_schema(format!("invalid {ty:?} value `{value}`")))
}

fn yaml_to_value(yaml: Yaml) -> Result<Value> {
    Ok(match yaml {
        Yaml::Null => Value::Null,
        Yaml::Bool(value) => Value::Bool(value),
        Yaml::Number(value) => {
            if let Some(value) = value.as_i64() {
                Value::I64(value)
            } else if let Some(value) = value.as_u64() {
                Value::U64(value)
            } else if let Some(value) = value.as_f64() {
                Value::F64(value)
            } else {
                return Err(Error::invalid_schema(format!(
                    "unsupported YAML number `{value}`"
                )));
            }
        }
        Yaml::String(value) => Value::String(value),
        Yaml::Sequence(items) => Value::List(
            items
                .into_iter()
                .map(yaml_to_value)
                .collect::<Result<_>>()?,
        ),
        Yaml::Mapping(_) => return yaml_to_object(yaml),
        Yaml::Tagged(tagged) => return yaml_to_value(tagged.value),
    })
}

fn yaml_to_object(yaml: Yaml) -> Result<Value> {
    let Yaml::Mapping(mapping) = yaml else {
        return type_error(&stmt::Type::Object, yaml);
    };
    let mut entries = Vec::with_capacity(mapping.len());
    for (key, value) in mapping {
        let Yaml::String(key) = key else {
            return Err(Error::invalid_schema(
                "document mappings require string keys",
            ));
        };
        entries.push((key, yaml_to_value(value)?));
    }
    Ok(Value::Object(ValueObject::from_vec(entries)))
}

fn type_error<T>(ty: &stmt::Type, yaml: Yaml) -> Result<T> {
    Err(Error::invalid_schema(format!(
        "expected {ty:?}, got YAML value {yaml:?}"
    )))
}

fn file_error(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |error| Error::from_args(format_args!("failed to read `{}`: {error}", path.display()))
}

fn invalid_utf8_path(path: &Path) -> Error {
    Error::invalid_driver_configuration(format!(
        "Markdown path `{}` is not valid UTF-8",
        path.display()
    ))
}

fn read_only_error() -> Error {
    Error::unsupported_feature("the Markdown driver is read-only")
}

#[async_trait]
impl Driver for Markdown {
    fn url(&self) -> Cow<'_, str> {
        Cow::Owned(format!("markdown:{}", self.root.display()))
    }

    fn capability(&self) -> &'static Capability {
        &toasty_driver_memory::CAPABILITY
    }

    async fn initialize(&mut self, schema: &Arc<Schema>) -> Result<()> {
        self.snapshot = Some(Arc::new(self.load(schema)?));
        Ok(())
    }

    async fn connect(&self, _cx: &ConnectContext) -> Result<Box<dyn toasty_core::Connection>> {
        let snapshot = self.snapshot.clone().ok_or_else(|| {
            Error::invalid_driver_configuration("Markdown driver was not initialized")
        })?;
        Ok(Box::new(Connection::new(snapshot)))
    }

    fn generate_migration(&self, _schema_diff: &diff::Schema<'_>) -> Migration {
        Migration::new_sql(String::new())
    }

    async fn reset_db(&self) -> Result<()> {
        Err(read_only_error())
    }
}
