use crate::{Error, Model};
use toasty_core::{stmt, Schema};

pub struct Cursor<'stmt, M> {
    schema: Schema,
    values: stmt::ValueStream<'stmt>,
    _p: std::marker::PhantomData<M>,
}

pub trait FromCursor<A>: Extend<A> + Default {}

impl<A, T: Extend<A> + Default> FromCursor<A> for T {}

impl<'stmt, M: Model> Cursor<'stmt, M> {
    pub(crate) fn new(schema: Schema, values: stmt::ValueStream<'stmt>) -> Cursor<'stmt, M> {
        Cursor {
            schema,
            values,
            _p: std::marker::PhantomData,
        }
    }

    pub async fn next(&mut self) -> Option<Result<M, Error>> {
        Some(match self.values.next().await? {
            Ok(stmt::Value::Record(row)) => {
                self.validate_row(&row);
                M::load(row.into_owned())
            }
            Ok(_) => todo!(),
            Err(e) => Err(e),
        })
    }

    /// Collect all values
    pub async fn collect<B>(mut self) -> Result<B, Error>
    where
        B: FromCursor<M>,
    {
        let mut ret = B::default();

        while let Some(res) = self.next().await {
            ret.extend(Some(res?));
        }

        Ok(ret)
    }

    #[track_caller]
    fn validate_row(&self, record: &stmt::Record<'_>) {
        if cfg!(debug_assertions) {
            let expect_num_columns = self.schema.model(M::ID).fields.len();

            if record.len() != expect_num_columns {
                panic!("expected row to have {expect_num_columns} columns; {record:#?}");
            }
        }
    }
}
