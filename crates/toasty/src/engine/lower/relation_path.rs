//! Helpers for resolving app-level relation paths.

use toasty_core::schema::app;

/// Walk a relation path, inlining each `via` field's resolved relation chain.
///
/// The result contains only direct relation fields. Scalar terminals are
/// omitted because they project a value from the model reached by the
/// relation chain; they do not add another relation step.
pub(super) fn flatten_relation_path(
    schema: &toasty_core::Schema,
    source_model_id: app::ModelId,
    initial_steps: &[usize],
) -> Vec<app::FieldId> {
    let mut result = Vec::with_capacity(initial_steps.len());
    let mut current_model = source_model_id;
    let mut queue: Vec<usize> = initial_steps.to_vec();
    queue.reverse();

    while let Some(index) = queue.pop() {
        let field = &schema.app.model(current_model).as_root_unwrap().fields[index];
        let field_id = app::FieldId {
            model: current_model,
            index,
        };

        if let app::FieldTy::Via(via) = &field.ty {
            debug_assert_eq!(via.path.root.as_model(), Some(current_model));

            for step in via_relation_steps(via).iter().rev() {
                queue.push(*step);
            }
            continue;
        }

        current_model = field
            .relation_target_id()
            .expect("relation path step is not a relation");
        result.push(field_id);
    }

    result
}

/// Return the direct relation fields that a `via` relation follows.
pub(super) fn flatten_via_path(
    schema: &toasty_core::Schema,
    via: &app::Via,
) -> Option<Vec<app::FieldId>> {
    let root = via.path.root.as_model()?;
    Some(flatten_relation_path(schema, root, via_relation_steps(via)))
}

fn via_relation_steps(via: &app::Via) -> &[usize] {
    let projection = via.path.projection.as_slice();
    match via.terminal {
        Some(_) => &projection[..projection.len() - 1],
        None => projection,
    }
}
