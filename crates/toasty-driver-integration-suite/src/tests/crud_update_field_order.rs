use crate::prelude::*;

/// Regression tests for https://github.com/tokio-rs/toasty/issues/200
///
/// The bug: when Assignments used IndexMap keyed by field offset, updating
/// fields in an order different from declaration order could cause values to be
/// assigned to the wrong column, producing type-parsing errors (e.g. writing a
/// String into an i64 column).

/// Update only the last declared field, leaving earlier fields untouched.
/// With offset-based IndexMap this could assign the value to field 0 instead.
#[driver_test(id(ID))]
pub async fn update_last_field_only(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        name: String,
        age: i64,
        bio: String,
    }

    let mut db = test.setup_db(models!(Profile)).await;

    let mut profile = Profile::create()
        .name("Alice")
        .age(30)
        .bio("hello")
        .exec(&mut db)
        .await?;

    // Update only the last field
    profile.update().bio("updated bio").exec(&mut db).await?;

    assert_eq!(profile.bio, "updated bio");
    assert_eq!(profile.name, "Alice");
    assert_eq!(profile.age, 30);

    // Verify round-trip through DB
    let reloaded = Profile::get_by_id(&mut db, &profile.id).await?;
    assert_eq!(reloaded.bio, "updated bio");
    assert_eq!(reloaded.name, "Alice");
    assert_eq!(reloaded.age, 30);

    Ok(())
}

/// Update fields in reverse declaration order. If offset mapping is wrong, the
/// String value ends up in the i64 column (or vice versa), causing a parse error.
#[driver_test(id(ID))]
pub async fn update_fields_reverse_order(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        #[auto]
        id: ID,

        first: String,
        second: i64,
        third: String,
    }

    let mut db = test.setup_db(models!(Record)).await;

    let mut rec = Record::create()
        .first("a")
        .second(1)
        .third("c")
        .exec(&mut db)
        .await?;

    // Update in reverse declaration order: third, then second, then first
    rec.update()
        .third("C")
        .second(2)
        .first("A")
        .exec(&mut db)
        .await?;

    assert_eq!(rec.first, "A");
    assert_eq!(rec.second, 2);
    assert_eq!(rec.third, "C");

    let reloaded = Record::get_by_id(&mut db, &rec.id).await?;
    assert_eq!(reloaded.first, "A");
    assert_eq!(reloaded.second, 2);
    assert_eq!(reloaded.third, "C");

    Ok(())
}

/// Update a single middle field among many typed fields. This is the simplest
/// trigger for the original bug: a single assignment at a high offset gets
/// misrouted to offset 0.
#[driver_test(id(ID))]
pub async fn update_middle_field_mixed_types(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Widget {
        #[key]
        #[auto]
        id: ID,

        label: String,
        count: i64,
        active: bool,
        description: String,
    }

    let mut db = test.setup_db(models!(Widget)).await;

    let mut widget = Widget::create()
        .label("w1")
        .count(10)
        .active(true)
        .description("original")
        .exec(&mut db)
        .await?;

    // Update only the i64 field (offset 2). With the old bug this value could
    // land in the String column at offset 1, causing a type error.
    widget.update().count(99).exec(&mut db).await?;

    assert_eq!(widget.count, 99);
    assert_eq!(widget.label, "w1");
    assert_eq!(widget.active, true);
    assert_eq!(widget.description, "original");

    let reloaded = Widget::get_by_id(&mut db, &widget.id).await?;
    assert_eq!(reloaded.count, 99);
    assert_eq!(reloaded.label, "w1");

    Ok(())
}

/// Query-based update with fields in non-declaration order. Exercises the
/// filter_by path rather than the instance-update path.
#[driver_test(id(ID))]
pub async fn query_update_non_declaration_order(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Entry {
        #[key]
        #[auto]
        id: ID,

        title: String,
        priority: i64,
        note: String,
    }

    let mut db = test.setup_db(models!(Entry)).await;

    let entry = Entry::create()
        .title("task")
        .priority(1)
        .note("n/a")
        .exec(&mut db)
        .await?;

    // Update via query in reverse order
    Entry::filter_by_id(entry.id)
        .update()
        .note("important")
        .priority(5)
        .title("urgent task")
        .exec(&mut db)
        .await?;

    let reloaded = Entry::get_by_id(&mut db, &entry.id).await?;
    assert_eq!(reloaded.title, "urgent task");
    assert_eq!(reloaded.priority, 5);
    assert_eq!(reloaded.note, "important");

    Ok(())
}

/// Successive single-field updates targeting different offsets each time.
/// Ensures that each individual update routes to the correct column.
#[driver_test(id(ID))]
pub async fn successive_single_field_updates(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        a: String,
        b: i64,
        c: String,
        d: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = Item::create()
        .a("a0")
        .b(0)
        .c("c0")
        .d(0)
        .exec(&mut db)
        .await?;

    // Update each field individually, in non-declaration order
    item.update().d(4).exec(&mut db).await?;
    assert_eq!(item.d, 4);

    item.update().b(2).exec(&mut db).await?;
    assert_eq!(item.b, 2);

    item.update().c("c1").exec(&mut db).await?;
    assert_eq!(item.c, "c1");

    item.update().a("a1").exec(&mut db).await?;
    assert_eq!(item.a, "a1");

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_struct!(reloaded, _ { a: "a1", b: 2, c: "c1", d: 4, .. });

    Ok(())
}
