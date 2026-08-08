use toasty_core::{Error, stmt};

/// A loaded/unloaded value slot shared by deferred fields and relation fields.
///
/// Encoding:
/// - `Value::Null` means the slot is unloaded.
/// - `Value::Record([value])` means the slot is loaded.
///
/// The one-field record wrapper keeps loaded `NULL` distinct from unloaded,
/// which matters for `Deferred<Option<T>>` and eventually nullable relations.
pub(crate) enum LazySlot<T> {
    Unloaded,
    Loaded(T),
}

pub(crate) fn loaded_expr(value: stmt::Expr) -> stmt::Expr {
    stmt::Expr::record([value])
}

pub(crate) fn decode<T>(
    value: stmt::Value,
    label: &'static str,
    load: impl FnOnce(stmt::Value) -> crate::Result<T>,
) -> crate::Result<LazySlot<T>> {
    match value {
        stmt::Value::Null => Ok(LazySlot::Unloaded),
        stmt::Value::Record(record) if record.fields.len() == 1 => {
            let mut iter = record.fields.into_iter();
            Ok(LazySlot::Loaded(load(iter.next().unwrap())?))
        }
        value => Err(Error::from_args(format_args!(
            "{label} decoder expected Null or single-field Record, got {value:?}"
        ))),
    }
}
