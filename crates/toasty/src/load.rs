use crate::stmt::List;
use crate::Error;
use toasty_core::stmt;

/// Load an instance of a type from a [`Value`][stmt::Value].
///
/// The value is expected to be a `Value::Record` containing the type's fields.
/// This trait is implemented by both root models and any other types that can
/// be deserialized from the database value representation.
///
/// The associated `Output` type allows marker types (like `List<M>`) to
/// specify a concrete return type. For sized types `Output = Self`; for
/// `List<M>`, `Output = Vec<M>`.
pub trait Load {
    type Output;
    fn load(value: stmt::Value) -> Result<Self::Output, Error>;
}

impl Load for () {
    type Output = ();
    fn load(_value: stmt::Value) -> Result<Self::Output, Error> {
        Ok(())
    }
}

impl Load for i64 {
    type Output = i64;
    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::Record(mut record) => match record.fields.remove(0) {
                stmt::Value::I64(n) => Ok(n),
                other => Err(Error::type_conversion(other, "i64")),
            },
            stmt::Value::I64(n) => Ok(n),
            _ => Err(Error::type_conversion(value, "i64")),
        }
    }
}

impl<T: Load<Output = T>> Load for Vec<T> {
    type Output = Vec<T>;
    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::List(items) => items.into_iter().map(T::load).collect(),
            // Records are produced by dynamic batch queries (Vec/array inputs)
            // where each field in the record is one query's result.
            stmt::Value::Record(record) => record.into_iter().map(T::load).collect(),
            _ => Err(Error::type_conversion(value, "Vec<T>")),
        }
    }
}

/// List type encoding: `List<M>` loads as `Vec<M::Output>`.
impl<M: Load> Load for List<M> {
    type Output = Vec<M::Output>;
    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::List(items) => items.into_iter().map(M::load).collect(),
            stmt::Value::Record(record) => record.into_iter().map(M::load).collect(),
            _ => Err(Error::type_conversion(value, "List<M>")),
        }
    }
}

macro_rules! impl_load_for_tuple {
    ( $( $T:ident ),+ ; $( $idx:tt ),+ ) => {
        impl< $( $T: Load ),+ > Load for ( $( $T, )+ ) {
            type Output = ( $( $T::Output, )+ );
            fn load(value: stmt::Value) -> Result<Self::Output, Error> {
                match value {
                    stmt::Value::Record(mut record) => Ok((
                        $( $T::load(record[$idx].take())?, )+
                    )),
                    _ => Err(Error::type_conversion(value, "tuple")),
                }
            }
        }
    };
}

impl_load_for_tuple!(A, B; 0, 1);
impl_load_for_tuple!(A, B, C; 0, 1, 2);
impl_load_for_tuple!(A, B, C, D; 0, 1, 2, 3);
impl_load_for_tuple!(A, B, C, D, E; 0, 1, 2, 3, 4);
impl_load_for_tuple!(A, B, C, D, E, F; 0, 1, 2, 3, 4, 5);
impl_load_for_tuple!(A, B, C, D, E, F, G; 0, 1, 2, 3, 4, 5, 6);
impl_load_for_tuple!(A, B, C, D, E, F, G, H; 0, 1, 2, 3, 4, 5, 6, 7);
