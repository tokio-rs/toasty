use crate::Error;
use toasty_core::stmt;

/// Load an instance of a type from a [`Value`][stmt::Value].
///
/// The value is expected to be a `Value::Record` containing the type's fields.
/// This trait is implemented by both root models and any other types that can
/// be deserialized from the database value representation.
pub trait Load: Sized {
    fn load(value: stmt::Value) -> Result<Self, Error>;
}

impl<T: Load> Load for Vec<T> {
    fn load(value: stmt::Value) -> Result<Self, Error> {
        match value {
            stmt::Value::List(items) => items.into_iter().map(T::load).collect(),
            _ => Err(Error::type_conversion(value, "Vec<T>")),
        }
    }
}

macro_rules! impl_load_for_tuple {
    ( $( $T:ident ),+ ; $( $idx:tt ),+ ) => {
        impl< $( $T: Load ),+ > Load for ( $( $T, )+ ) {
            fn load(value: stmt::Value) -> Result<Self, Error> {
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
