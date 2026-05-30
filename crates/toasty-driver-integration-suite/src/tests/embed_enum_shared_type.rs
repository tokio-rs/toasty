use crate::prelude::*;

/// The same `#[derive(Embed)]` enum used as a field in two different models.
///
/// On PostgreSQL, both tables should share the same `CREATE TYPE priority …`
/// enum type. This test verifies that CRUD and filter operations work correctly
/// when the enum type is reused across tables.
#[driver_test(id(ID), scenario(crate::scenarios::task_bug_priority))]
pub async fn shared_enum_crud(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Create records in both tables
    let task = toasty::create!(Task {
        title: "ship it",
        priority: Priority::High,
    })
    .exec(&mut db)
    .await?;

    let bug = toasty::create!(Bug {
        summary: "papercut",
        priority: Priority::Low,
    })
    .exec(&mut db)
    .await?;

    // Read back
    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.priority,
        Priority::High
    );
    assert_eq!(
        Bug::get_by_id(&mut db, &bug.id).await?.priority,
        Priority::Low
    );

    // Update each independently
    let mut task = task;
    task.update()
        .priority(Priority::Medium)
        .exec(&mut db)
        .await?;
    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.priority,
        Priority::Medium
    );

    let mut bug = bug;
    bug.update().priority(Priority::High).exec(&mut db).await?;
    assert_eq!(
        Bug::get_by_id(&mut db, &bug.id).await?.priority,
        Priority::High
    );

    Ok(())
}

/// Filter by enum value on two models that share the same enum type.
#[driver_test(id(ID), requires(scan), scenario(crate::scenarios::task_bug_priority))]
pub async fn shared_enum_filter(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Seed data
    for (title, p) in [
        ("a", Priority::Low),
        ("b", Priority::Medium),
        ("c", Priority::High),
        ("d", Priority::High),
    ] {
        toasty::create!(Task { title, priority: p })
            .exec(&mut db)
            .await?;
    }

    for (summary, p) in [
        ("x", Priority::Low),
        ("y", Priority::Low),
        ("z", Priority::Medium),
    ] {
        toasty::create!(Bug {
            summary,
            priority: p
        })
        .exec(&mut db)
        .await?;
    }

    // Filter tasks by enum value
    let high_tasks = Task::filter(Task::fields().priority().is_high())
        .exec(&mut db)
        .await?;
    assert_eq!(high_tasks.len(), 2);

    // Filter bugs by enum value
    let low_bugs = Bug::filter(Bug::fields().priority().is_low())
        .exec(&mut db)
        .await?;
    assert_eq!(low_bugs.len(), 2);

    // ne filter on each table
    let not_low_tasks = Task::filter(Task::fields().priority().ne(Priority::Low))
        .exec(&mut db)
        .await?;
    assert_eq!(not_low_tasks.len(), 3);

    let not_medium_bugs = Bug::filter(Bug::fields().priority().ne(Priority::Medium))
        .exec(&mut db)
        .await?;
    assert_eq!(not_medium_bugs.len(), 2);

    Ok(())
}
