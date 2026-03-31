use crate::stmt::List;
use toasty_core::Error;
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
    /// The concrete type produced by [`load`](Self::load).
    ///
    /// For sized types this is `Self`. For marker types like [`List<M>`](crate::stmt::List),
    /// it is `Vec<M::Output>`.
    type Output;

    /// Returns the [`stmt::Type`] that describes values of this type.
    fn ty() -> stmt::Type;

    /// Returns the [`stmt::Type`] used when this type appears as a relation
    /// target. The default delegates to [`ty()`](Load::ty).
    fn ty_relation() -> stmt::Type {
        Self::ty()
    }

    /// Deserialize a database [`Value`](stmt::Value) into `Self::Output`.
    ///
    /// Returns an error if the value cannot be converted to this type.
    fn load(value: stmt::Value) -> Result<Self::Output, Error>;

    /// Deserialize a value that was loaded as a relation target.
    ///
    /// The default delegates to [`load`](Self::load). Override this when
    /// relation values use a different encoding (e.g., `Option<T>` uses a
    /// sentinel integer to distinguish "loaded as None" from "not loaded").
    fn load_relation(value: stmt::Value) -> Result<Self::Output, Error> {
        Self::load(value)
    }

    /// Reload the value in-place from a value returned by the database.
    ///
    /// The value may be a `SparseRecord` for partial embedded updates, in which
    /// case only the specified fields should be updated. Embedded types must
    /// override this method to handle partial updates correctly.
    ///
    /// Takes `&mut Self::Output` rather than `&mut self` so that wrapper types
    /// like `Option<T>` can implement reload generically regardless of whether
    /// `T::Output == T`.
    ///
    /// The default implementation panics. Types that support reloading (i.e.,
    /// types that implement [`Field`]) should override this.
    fn reload(target: &mut Self::Output, value: stmt::Value) -> Result<(), Error> {
        let _ = (target, value);
        unimplemented!("reload is not supported for this type")
    }
}

impl Load for () {
    type Output = ();

    fn ty() -> stmt::Type {
        stmt::Type::Unit
    }

    fn load(_value: stmt::Value) -> Result<Self::Output, Error> {
        Ok(())
    }
}

impl<T: Load<Output = T>> Load for Vec<T> {
    type Output = Vec<T>;

    fn ty() -> stmt::Type {
        stmt::Type::list(T::ty())
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::List(items) => items.into_iter().map(T::load).collect(),
            // Records are produced by dynamic batch queries (Vec/array inputs)
            // where each field in the record is one query's result.
            stmt::Value::Record(record) => record.into_iter().map(T::load).collect(),
            // Bytes are a compact representation; load each byte individually.
            stmt::Value::Bytes(bytes) => bytes
                .into_iter()
                .map(|b| T::load(stmt::Value::U8(b)))
                .collect(),
            _ => Err(Error::type_conversion(value, "Vec<T>")),
        }
    }

    fn reload(target: &mut Self::Output, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

/// List type encoding: `List<M>` loads as `Vec<M::Output>`.
impl<M: Load> Load for List<M> {
    type Output = Vec<M::Output>;

    fn ty() -> stmt::Type {
        stmt::Type::list(M::ty())
    }

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

            fn ty() -> stmt::Type {
                stmt::Type::Record(vec![ $( $T::ty() ),+ ])
            }

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

impl Load for String {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::String(v) => Ok(v),
            _ => Err(Error::type_conversion(value, "String")),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Load for uuid::Uuid {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Uuid
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::Uuid(v) => Ok(v),
            _ => Err(Error::type_conversion(value, "uuid::Uuid")),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Load for bool {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Bool
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::Bool(v) => Ok(v),
            _ => Err(Error::type_conversion(value, "bool")),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T> Load for std::borrow::Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Load<Output = T::Owned>,
{
    type Output = Self;

    fn ty() -> stmt::Type {
        <T::Owned as Load>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        <T::Owned as Load>::load(value).map(std::borrow::Cow::Owned)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: Load<Output = T>> Load for std::sync::Arc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        <T as Load>::load(value).map(std::sync::Arc::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: Load<Output = T>> Load for std::rc::Rc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        <T as Load>::load(value).map(std::rc::Rc::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: Load<Output = T>> Load for Box<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        <T as Load>::load(value).map(Box::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

// Pointer-sized integers map to fixed-size types internally
impl Load for isize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::I64
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        value.try_into()
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl Load for usize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        value.try_into()
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "rust_decimal")]
impl Load for rust_decimal::Decimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Decimal
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::Decimal(v) => Ok(v),
            _ => Err(Error::type_conversion(value, "rust_decimal::Decimal")),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "bigdecimal")]
impl Load for bigdecimal::BigDecimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::BigDecimal
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::BigDecimal(v) => Ok(v),
            _ => Err(Error::type_conversion(value, "bigdecimal::BigDecimal")),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<(), Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}
