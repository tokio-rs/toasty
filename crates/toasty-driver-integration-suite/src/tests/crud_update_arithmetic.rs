use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{Assignment, Statement, UpdateTarget},
};

#[driver_test(scenario(crate::scenarios::counter_value))]
pub async fn arithmetic_update_cases(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;

    counter
        .update()
        .value(toasty::stmt::increment())
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 11);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    counter
        .update()
        .value(toasty::stmt::decrement())
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 9);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    counter
        .update()
        .value(toasty::stmt::add(25))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 35);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    counter
        .update()
        .value(toasty::stmt::subtract(3))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 7);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    counter
        .update()
        .value(toasty::stmt::add(-4))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 6);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    // Column index 1 is `value`. Confirm increment lowers to `Assignment::Add`;
    // the value checks above cover each driver's return path.
    let counter_table_id = table_id(&db, "counters");
    let is_sql = t.capability().sql;
    t.log().clear();
    counter
        .update()
        .value(toasty::stmt::increment())
        .exec(&mut db)
        .await?;
    let (op, _resp) = t.log().pop();
    if is_sql {
        assert_struct!(op, Operation::QuerySql({
            stmt: Statement::Update({
                target: UpdateTarget::Table(== counter_table_id),
                assignments: #{ [1]: Assignment::Add(_) },
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey({
            table: == counter_table_id,
            assignments: #{ [1]: Assignment::Add(_) },
            ..
        }));
    }

    // Duplicate assignments once crashed lowering; they must fold into one
    // arithmetic update.
    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    counter
        .update()
        .value(toasty::stmt::add(2))
        .value(toasty::stmt::add(3))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 15);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    // A leading subtract flips later add operands in the batch fold.
    counter
        .update()
        .value(toasty::stmt::subtract(3))
        .value(toasty::stmt::add(7))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 14);

    let mut counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    // Set clobbers prior state and absorbs subsequent arithmetic.
    counter
        .update()
        .value(50)
        .value(toasty::stmt::add(8))
        .value(toasty::stmt::subtract(3))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 55);

    let counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await?;
    Counter::filter_by_id(counter.id)
        .update()
        .value(toasty::stmt::add(100))
        .exec(&mut db)
        .await?;
    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 110);

    Ok(())
}

#[driver_test]
pub async fn arithmetic_chains_with_other_updates(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        id: uuid::Uuid,

        name: String,
        login_count: i64,
    }

    let mut db = t.setup_db(models!(Profile)).await;
    let mut profile = toasty::create!(Profile {
        id: uuid::Uuid::new_v4(),
        name: "alice",
        login_count: 5,
    })
    .exec(&mut db)
    .await?;

    profile
        .update()
        .name("alice2")
        .login_count(toasty::stmt::increment())
        .exec(&mut db)
        .await?;

    let reloaded = Profile::get_by_id(&mut db, &profile.id).await?;
    assert_struct!(reloaded, _ { name: "alice2", login_count: 6, .. });
    Ok(())
}

#[driver_test]
pub async fn increment_unique_column(t: &mut Test) -> Result<()> {
    // Regression: DynamoDB's unique-index update path assumes every
    // assignment on a unique column is `Set` (`let Set(expr) = assignment
    // else unreachable!()`). Filtering by projection alone lets `Add` /
    // `Subtract` reach the let-else and panic. Incrementing a unique
    // numeric column should succeed on every backend.
    #[derive(Debug, toasty::Model)]
    struct Slot {
        #[key]
        id: uuid::Uuid,

        #[unique]
        count: i64,
    }

    let mut db = t.setup_db(models!(Slot)).await;
    let mut slot = toasty::create!(Slot {
        id: uuid::Uuid::new_v4(),
        count: 10,
    })
    .exec(&mut db)
    .await?;

    slot.update()
        .count(toasty::stmt::increment())
        .exec(&mut db)
        .await?;

    let reloaded = Slot::get_by_id(&mut db, &slot.id).await?;
    assert_eq!(reloaded.count, 11);
    Ok(())
}

#[driver_test]
pub async fn increment_narrow_integer_column(t: &mut Test) -> Result<()> {
    // Regression: stmt::increment() / stmt::decrement() hardcoded
    // Value::I64(1), which the PostgreSQL driver had no `(I64, INT2)` arm
    // for — incrementing an `i8`, `i16`, or `u8` column panicked at value
    // binding. The literal must encode in a value variant that fits any
    // integer column on every backend.
    #[derive(Debug, toasty::Model)]
    struct Tally {
        #[key]
        id: uuid::Uuid,

        count: i16,
    }

    let mut db = t.setup_db(models!(Tally)).await;
    let mut tally = toasty::create!(Tally {
        id: uuid::Uuid::new_v4(),
        count: 10_i16,
    })
    .exec(&mut db)
    .await?;

    tally
        .update()
        .count(toasty::stmt::increment())
        .exec(&mut db)
        .await?;
    assert_eq!(Tally::get_by_id(&mut db, &tally.id).await?.count, 11);

    tally
        .update()
        .count(toasty::stmt::decrement())
        .exec(&mut db)
        .await?;
    assert_eq!(Tally::get_by_id(&mut db, &tally.id).await?.count, 10);

    Ok(())
}
