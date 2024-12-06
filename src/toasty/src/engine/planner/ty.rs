use super::*;

pub(crate) fn value(value: &stmt::Value) -> stmt::Type {
    todo!("value={value:#?}")
}

pub(crate) fn model_record(model: &Model) -> stmt::Type {
    stmt::Type::Record(
        model
            .fields
            .iter()
            .map(|field| field.expr_ty().clone())
            .collect(),
    )
}
