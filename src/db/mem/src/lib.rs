mod config;
pub use config::DriverConfig;

use toasty_driver::schema::{ModelId, Schema};
use toasty_driver::{op, Driver, Error, ResultFuture, RowSet, Value};

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug)]
pub struct MemDriver {
    config: DriverConfig,
    store: Mutex<Store>,
}

#[derive(Debug)]
struct Store {
    models: HashMap<ModelId, Vec<Row>>,
}

#[derive(Debug, Default)]
struct Row(Vec<Value>);

impl MemDriver {
    pub fn new() -> MemDriver {
        MemDriver {
            config: DriverConfig::default(),
            store: Mutex::new(Store {
                models: HashMap::new(),
            }),
        }
    }
}

impl Driver for MemDriver {
    fn register_schema<'a>(&'a mut self, op: &op::RegisterSchema) -> ResultFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn create_record<'a>(&'a self, op: &op::CreateRecord) -> ResultFuture<'a, ()> {
        let row = op
            .fields
            .iter()
            .map(|value_ref| value_ref.to_owned())
            .collect::<Vec<_>>();

        let mut store = self.store.lock().unwrap();
        let rows = store.models.entry(op.model).or_default();

        // Store the row
        rows.push(Row(row));

        Box::pin(async { Ok(()) })
    }

    /// Query the database
    fn query_many<'a>(&'a self, op: &op::QueryMany) -> Box<dyn RowSet> {
        todo!()
    }
}
