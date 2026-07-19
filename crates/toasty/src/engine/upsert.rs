use toasty_core::{Result, stmt};

/// Converts declarative upsert branches into an insert row and one conflict
/// assignment map before model fields are lowered to database columns.
pub(super) fn normalize(insert: &mut stmt::Insert, preserve_initializers: bool) -> Result<()> {
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

    for (projection, assignment) in &upsert.initializers {
        let stmt::Assignment::Set(expr) = assignment else {
            return Err(crate::Error::invalid_statement(
                "upsert field initializers only support value assignments",
            ));
        };
        set_create_value(record, projection, expr.clone())?;
    }

    for (projection, assignment) in &upsert.shared {
        if upsert.create.contains(projection) {
            continue;
        }
        let initializer = upsert.initializers.get(projection);
        let expr = create_expr_for_assignment(assignment, initializer)?;
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

    if !preserve_initializers {
        upsert.initializers = stmt::Assignments::new();
    }
    upsert.defaulted.clear();

    Ok(())
}

fn create_expr_for_assignment(
    assignment: &stmt::Assignment,
    initializer: Option<&stmt::Assignment>,
) -> Result<stmt::Expr> {
    match assignment {
        stmt::Assignment::Set(expr) => Ok(expr.clone()),
        assignment if !assignment.requires_current_value() => {
            Ok(stmt::Expr::Value(apply_assignment(None, assignment)?))
        }
        assignment => {
            let initializer = initializer.ok_or_else(|| {
                crate::Error::invalid_statement(
                    "shared upsert mutations require a field with #[default]",
                )
            })?;
            let stmt::Assignment::Set(initializer) = initializer else {
                return Err(crate::Error::invalid_statement(
                    "upsert field initializers only support value assignments",
                ));
            };
            let value = apply_assignment(Some(initializer.eval_const()?), assignment)?;
            Ok(stmt::Expr::Value(value))
        }
    }
}

fn apply_assignment(
    current: Option<stmt::Value>,
    assignment: &stmt::Assignment,
) -> Result<stmt::Value> {
    use stmt::Assignment::*;

    match assignment {
        Set(expr) => expr.eval_const(),
        Insert(expr) => {
            let mut items = current_list(current)?;
            items.push(expr.eval_const()?);
            Ok(stmt::Value::List(items))
        }
        Remove(expr) => {
            let mut items = current_list(current)?;
            let removed = expr.eval_const()?;
            items.retain(|item| item != &removed);
            Ok(stmt::Value::List(items))
        }
        Append(expr) => {
            let mut items = current_list(current)?;
            let stmt::Value::List(appended) = expr.eval_const()? else {
                return Err(crate::Error::invalid_statement(
                    "upsert append assignment requires a list value",
                ));
            };
            items.extend(appended);
            Ok(stmt::Value::List(items))
        }
        Pop => {
            let mut items = current_list(current)?;
            items.pop();
            Ok(stmt::Value::List(items))
        }
        RemoveAt(expr) => {
            let mut items = current_list(current)?;
            let index = usize::try_from(expr.eval_const()?)?;
            if index < items.len() {
                items.remove(index);
            }
            Ok(stmt::Value::List(items))
        }
        Add(expr) => {
            let current = current_value(current)?;
            current.checked_add(&expr.eval_const()?).ok_or_else(|| {
                crate::Error::invalid_statement(
                    "upsert add assignment overflowed or used incompatible numeric types",
                )
            })
        }
        Subtract(expr) => {
            let current = current_value(current)?;
            current.checked_sub(&expr.eval_const()?).ok_or_else(|| {
                crate::Error::invalid_statement(
                    "upsert subtract assignment overflowed or used incompatible numeric types",
                )
            })
        }
        Batch(assignments) => {
            let mut current = current;
            for assignment in assignments {
                current = Some(apply_assignment(current, assignment)?);
            }
            current_value(current)
        }
    }
}

fn current_value(current: Option<stmt::Value>) -> Result<stmt::Value> {
    current.ok_or_else(|| {
        crate::Error::invalid_statement("shared upsert mutations require a field with #[default]")
    })
}

fn current_list(current: Option<stmt::Value>) -> Result<Vec<stmt::Value>> {
    let stmt::Value::List(items) = current_value(current)? else {
        return Err(crate::Error::invalid_statement(
            "upsert collection assignment requires a list default",
        ));
    };
    Ok(items)
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
