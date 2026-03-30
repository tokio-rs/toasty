use crate::prelude::*;

/// Regression tests for https://github.com/tokio-rs/toasty/issues/200
///
/// The bug: when Assignments used IndexMap keyed by field offset, updating
/// fields in an order different from declaration order could cause values to be
/// assigned to the wrong column, producing type-parsing errors (e.g. writing a
/// String into an i64 column).

/// Update only the last declared field, leaving earlier fields untouched.
/// With offset-based IndexMap this could assign the value to field 0 instead.
#[driver_test(id(ID), scenario(crate::scenarios::widget_mixed_types))]
pub async fn update_last_field_only(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut w = Widget::create()
        .label("w1")
        .count(10)
        .active(true)
        .description("hello")
        .exec(&mut db)
        .await?;

    w.update().description("updated").exec(&mut db).await?;

    assert_struct!(w, _ { label: "w1", count: 10, active: true, description: "updated", .. });

    let reloaded = Widget::get_by_id(&mut db, &w.id).await?;
    assert_struct!(reloaded, _ { label: "w1", count: 10, active: true, description: "updated", .. });

    Ok(())
}

/// Update fields in reverse declaration order. If offset mapping is wrong, the
/// String value ends up in the i64 column (or vice versa), causing a parse error.
#[driver_test(id(ID), scenario(crate::scenarios::widget_mixed_types))]
pub async fn update_fields_reverse_order(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut w = Widget::create()
        .label("a")
        .count(1)
        .active(false)
        .description("c")
        .exec(&mut db)
        .await?;

    // Update in reverse declaration order
    w.update()
        .description("C")
        .active(true)
        .count(2)
        .label("A")
        .exec(&mut db)
        .await?;

    assert_struct!(w, _ { label: "A", count: 2, active: true, description: "C", .. });

    let reloaded = Widget::get_by_id(&mut db, &w.id).await?;
    assert_struct!(reloaded, _ { label: "A", count: 2, active: true, description: "C", .. });

    Ok(())
}

/// Update a single middle field among many typed fields. This is the simplest
/// trigger for the original bug: a single assignment at a high offset gets
/// misrouted to offset 0.
#[driver_test(id(ID), scenario(crate::scenarios::widget_mixed_types))]
pub async fn update_middle_field_mixed_types(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut w = Widget::create()
        .label("w1")
        .count(10)
        .active(true)
        .description("original")
        .exec(&mut db)
        .await?;

    // Update only the i64 field (offset 2). With the old bug this value could
    // land in the String column at offset 1, causing a type error.
    w.update().count(99).exec(&mut db).await?;

    assert_struct!(w, _ { label: "w1", count: 99, active: true, description: "original", .. });

    let reloaded = Widget::get_by_id(&mut db, &w.id).await?;
    assert_struct!(reloaded, _ { label: "w1", count: 99, .. });

    Ok(())
}

/// Query-based update with fields in non-declaration order. Exercises the
/// filter_by path rather than the instance-update path.
#[driver_test(id(ID), scenario(crate::scenarios::widget_mixed_types))]
pub async fn query_update_non_declaration_order(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let w = Widget::create()
        .label("task")
        .count(1)
        .active(false)
        .description("n/a")
        .exec(&mut db)
        .await?;

    // Update via query in reverse order
    Widget::filter_by_id(w.id)
        .update()
        .description("important")
        .active(true)
        .count(5)
        .label("urgent")
        .exec(&mut db)
        .await?;

    let reloaded = Widget::get_by_id(&mut db, &w.id).await?;
    assert_struct!(
        reloaded,
        _ { label: "urgent", count: 5, active: true, description: "important", .. }
    );

    Ok(())
}

/// Successive single-field updates targeting different offsets each time.
/// Ensures that each individual update routes to the correct column.
#[driver_test(id(ID), scenario(crate::scenarios::widget_mixed_types))]
pub async fn successive_single_field_updates(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut w = Widget::create()
        .label("a0")
        .count(0)
        .active(false)
        .description("d0")
        .exec(&mut db)
        .await?;

    // Update each field individually, in non-declaration order
    w.update().description("d1").exec(&mut db).await?;
    assert_eq!(w.description, "d1");

    w.update().count(2).exec(&mut db).await?;
    assert_eq!(w.count, 2);

    w.update().active(true).exec(&mut db).await?;
    assert_eq!(w.active, true);

    w.update().label("a1").exec(&mut db).await?;
    assert_eq!(w.label, "a1");

    let reloaded = Widget::get_by_id(&mut db, &w.id).await?;
    assert_struct!(reloaded, _ { label: "a1", count: 2, active: true, description: "d1", .. });

    Ok(())
}
