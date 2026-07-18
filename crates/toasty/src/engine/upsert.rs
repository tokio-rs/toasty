use toasty_core::{Result, stmt};

/// Converts declarative upsert branches into an insert row and one conflict
/// assignment map before model fields are lowered to database columns.
pub(super) fn normalize(insert: &mut stmt::Insert) -> Result<()> {
    let Some(upsert) = &mut insert.upsert else {
        return Ok(());
    };

    let stmt::ExprSet::Values(values) = &mut insert.source.body else {
        return Err(crate::Error::invalid_statement(
            "upsert branches require a VALUES source",
        ));
    };
    let [row] = values.rows.as_mut_slice() else {
        return Err(crate::Error::invalid_statement(
            "upsert branches require exactly one source row",
        ));
    };
    if !row.is_record() {
        let Some(fields) = row.take().into_record_items() else {
            return Err(crate::Error::invalid_statement(
                "upsert branches require a record source row",
            ));
        };
        *row = stmt::Expr::record(fields);
    }
    let Some(record) = row.as_record_mut() else {
        return Err(crate::Error::invalid_statement(
            "upsert branches require a record source row",
        ));
    };

    for (projection, assignment) in &upsert.shared {
        if upsert.create.contains(projection) {
            continue;
        }
        let Some(expr) = create_expr_for_assignment(assignment) else {
            return Err(crate::Error::invalid_statement(
                "upsert assignment cannot initialize the field on the create branch; use on_create and on_update instead",
            ));
        };
        set_create_value(record, projection, expr)?;
    }

    for (projection, assignment) in std::mem::take(&mut upsert.create) {
        let stmt::Assignment::Set(expr) = assignment else {
            return Err(crate::Error::invalid_statement(
                "upsert create branch only supports value assignments",
            ));
        };
        set_create_value(record, &projection, expr)?;
    }

    if upsert.action == stmt::UpsertAction::Update {
        upsert.shared.overlay(std::mem::take(&mut upsert.update));
    } else {
        upsert.shared = stmt::Assignments::new();
        upsert.update = stmt::Assignments::new();
    }

    Ok(())
}

pub(super) fn create_expr_for_assignment(assignment: &stmt::Assignment) -> Option<stmt::Expr> {
    match assignment {
        stmt::Assignment::Set(expr)
        | stmt::Assignment::Append(expr)
        | stmt::Assignment::Add(expr) => Some(expr.clone()),
        stmt::Assignment::Subtract(stmt::Expr::Value(value)) => {
            negate_numeric(value).map(stmt::Expr::Value)
        }
        _ => None,
    }
}

fn set_create_value(
    record: &mut stmt::ExprRecord,
    projection: &stmt::Projection,
    expr: stmt::Expr,
) -> Result<()> {
    let [field] = projection.as_slice() else {
        return Err(crate::Error::invalid_statement(
            "upsert create assignments must target one model field",
        ));
    };
    let Some(slot) = record.fields.get_mut(*field) else {
        return Err(crate::Error::invalid_statement(
            "upsert create assignment targets an unknown model field",
        ));
    };
    *slot = expr;
    Ok(())
}

fn negate_numeric(value: &stmt::Value) -> Option<stmt::Value> {
    match value {
        stmt::Value::I8(value) => value.checked_neg().map(stmt::Value::I8),
        stmt::Value::I16(value) => value.checked_neg().map(stmt::Value::I16),
        stmt::Value::I32(value) => value.checked_neg().map(stmt::Value::I32),
        stmt::Value::I64(value) => value.checked_neg().map(stmt::Value::I64),
        stmt::Value::F32(value) => Some(stmt::Value::F32(-value)),
        stmt::Value::F64(value) => Some(stmt::Value::F64(-value)),
        _ => None,
    }
}
