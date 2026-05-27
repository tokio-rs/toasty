use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{Assignment, Statement, UpdateTarget},
};

#[derive(Debug, toasty::Model)]
struct Counter {
    #[key]
    id: uuid::Uuid,

    value: i64,
}

async fn setup(t: &mut Test) -> (toasty::Db, Counter) {
    let mut db = t.setup_db(models!(Counter)).await;
    let counter = toasty::create!(Counter {
        id: uuid::Uuid::new_v4(),
        value: 10,
    })
    .exec(&mut db)
    .await
    .unwrap();
    (db, counter)
}

#[driver_test]
pub async fn increment_adds_one(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::increment())
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 11);
    Ok(())
}

#[driver_test]
pub async fn decrement_subtracts_one(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::decrement())
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 9);
    Ok(())
}

#[driver_test]
pub async fn add_adds_value(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::add(25))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 35);
    Ok(())
}

#[driver_test]
pub async fn subtract_subtracts_value(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::subtract(3))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 7);
    Ok(())
}

#[driver_test]
pub async fn add_negative_value(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::add(-4))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 6);
    Ok(())
}

#[driver_test]
pub async fn increment_emits_add_assignment(t: &mut Test) -> Result<()> {
    let (mut db, mut counter) = setup(t).await;

    let counter_table_id = table_id(&db, "counters");
    let is_sql = t.capability().sql;

    t.log().clear();
    counter
        .update()
        .value(toasty::stmt::increment())
        .exec(&mut db)
        .await?;

    let (op, _resp) = t.log().pop();
    // Column index 1 = value (id=0, value=1). Confirms the engine emits
    // an `Assignment::Add` for `stmt::increment()`. Driver-specific shape
    // (RETURNING vs follow-up SELECT) is exercised in the value-check
    // tests above; here we only care about the assignment variant.
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
pub async fn multiple_add_on_one_field(t: &mut Test) -> Result<()> {
    // Regression: chaining two arithmetic ops on the same field used to crash
    // lowering. `Assignments::add` batches duplicate keys into
    // `Assignment::Batch([Add, Add])`, and `fold_append_batch` only handles
    // `Append`. Two `stmt::add` on the same field should compose to a single
    // net add of (2 + 3).
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::add(2))
        .value(toasty::stmt::add(3))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 15);
    Ok(())
}

#[driver_test]
pub async fn subtract_then_add_on_one_field(t: &mut Test) -> Result<()> {
    // Covers the sign-flip path in the arithmetic-batch fold: when
    // `Subtract` leads, subsequent `Add` operands must flip to subtraction
    // inside the running operand (`col - a + b = col - (a - b)`).
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(toasty::stmt::subtract(3))
        .value(toasty::stmt::add(7))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 14);
    Ok(())
}

#[driver_test]
pub async fn set_then_arithmetic_on_one_field(t: &mut Test) -> Result<()> {
    // Covers the Set+arithmetic fold: a literal write followed by an
    // arithmetic op on the same field should reduce to `Set(literal ± op)`
    // — Set clobbers prior state and absorbs subsequent arithmetic.
    let (mut db, mut counter) = setup(t).await;

    counter
        .update()
        .value(50)
        .value(toasty::stmt::add(8))
        .value(toasty::stmt::subtract(3))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 55);
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
pub async fn filter_update_with_arithmetic(t: &mut Test) -> Result<()> {
    let (mut db, counter) = setup(t).await;

    Counter::filter_by_id(counter.id)
        .update()
        .value(toasty::stmt::add(100))
        .exec(&mut db)
        .await?;

    let reloaded = Counter::get_by_id(&mut db, &counter.id).await?;
    assert_eq!(reloaded.value, 110);
    Ok(())
}
