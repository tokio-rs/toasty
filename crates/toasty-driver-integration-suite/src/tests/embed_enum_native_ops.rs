use crate::prelude::*;

/// Full CRUD lifecycle for native database enums (string discriminant enums that
/// map to PostgreSQL CREATE TYPE, MySQL ENUM(), SQLite TEXT+CHECK).
///
/// Exercises INSERT, SELECT, UPDATE (instance and query-based), and DELETE
/// through the driver — the paths affected by parameter type inference.
#[driver_test(id(ID))]
pub async fn native_enum_crud_lifecycle(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Priority {
        Low,
        Medium,
        High,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: ID,
        title: String,
        priority: Priority,
    }

    let mut db = t.setup_db(models!(Task, Priority)).await;

    // -- Create with each variant --
    let t1 = toasty::create!(Task {
        title: "A",
        priority: Priority::Low
    })
    .exec(&mut db)
    .await?;
    let t2 = toasty::create!(Task {
        title: "B",
        priority: Priority::Medium
    })
    .exec(&mut db)
    .await?;
    let t3 = toasty::create!(Task {
        title: "C",
        priority: Priority::High
    })
    .exec(&mut db)
    .await?;

    assert_eq!(t1.priority, Priority::Low);
    assert_eq!(t2.priority, Priority::Medium);
    assert_eq!(t3.priority, Priority::High);

    // -- Get by ID roundtrips each variant --
    assert_eq!(
        Task::get_by_id(&mut db, &t1.id).await?.priority,
        Priority::Low
    );
    assert_eq!(
        Task::get_by_id(&mut db, &t2.id).await?.priority,
        Priority::Medium
    );
    assert_eq!(
        Task::get_by_id(&mut db, &t3.id).await?.priority,
        Priority::High
    );

    // -- Update via instance method --
    let mut t1 = t1;
    t1.update().priority(Priority::High).exec(&mut db).await?;
    assert_eq!(
        Task::get_by_id(&mut db, &t1.id).await?.priority,
        Priority::High
    );

    // -- Update via query --
    Task::filter_by_id(&t2.id)
        .update()
        .priority(Priority::Low)
        .exec(&mut db)
        .await?;
    assert_eq!(
        Task::get_by_id(&mut db, &t2.id).await?.priority,
        Priority::Low
    );

    // -- Delete and verify --
    t3.delete().exec(&mut db).await?;
    let all = Task::all().exec(&mut db).await?;
    assert_eq!(all.len(), 2);

    Ok(())
}

/// Filter operations on native database enums: eq, ne, in_list.
#[driver_test(requires(sql))]
pub async fn native_enum_filter_operations(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        Pending,
        Active,
        Done,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    for (name, status) in [
        ("A", Status::Pending),
        ("B", Status::Active),
        ("C", Status::Active),
        ("D", Status::Done),
    ] {
        toasty::create!(Task { name, status }).exec(&mut db).await?;
    }

    // -- Filter eq (via is_variant) --
    let active = Task::filter(Task::fields().status().is_active())
        .exec(&mut db)
        .await?;
    assert_eq!(active.len(), 2);

    // -- Filter ne --
    let not_active = Task::filter(Task::fields().status().ne(Status::Active))
        .exec(&mut db)
        .await?;
    assert_eq!(not_active.len(), 2);
    assert!(not_active.iter().all(|t| t.status != Status::Active));

    // -- Filter in_list --
    let pending_or_done = Task::filter(
        Task::fields()
            .status()
            .in_list([Status::Pending, Status::Done]),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(pending_or_done.len(), 2);
    assert!(pending_or_done.iter().all(|t| t.status != Status::Active));

    // -- Delete filtered by enum value --
    Task::filter(Task::fields().status().is_done())
        .delete()
        .exec(&mut db)
        .await?;
    let remaining = Task::all().exec(&mut db).await?;
    assert_eq!(remaining.len(), 3);

    Ok(())
}

/// Multiple native enum fields on the same model, each creating its own
/// database enum type.
#[driver_test(id(ID))]
pub async fn native_enum_multiple_fields(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Priority {
        Low,
        High,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        Open,
        Closed,
    }

    #[derive(Debug, toasty::Model)]
    struct Ticket {
        #[key]
        #[auto]
        id: ID,
        priority: Priority,
        status: Status,
    }

    let mut db = t.setup_db(models!(Ticket, Priority, Status)).await;

    let ticket = toasty::create!(Ticket {
        priority: Priority::High,
        status: Status::Open,
    })
    .exec(&mut db)
    .await?;

    let found = Ticket::get_by_id(&mut db, &ticket.id).await?;
    assert_eq!(found.priority, Priority::High);
    assert_eq!(found.status, Status::Open);

    // Update both enum fields
    let mut ticket = found;
    ticket
        .update()
        .priority(Priority::Low)
        .status(Status::Closed)
        .exec(&mut db)
        .await?;

    let found = Ticket::get_by_id(&mut db, &ticket.id).await?;
    assert_eq!(found.priority, Priority::Low);
    assert_eq!(found.status, Status::Closed);

    Ok(())
}

/// Native enum with explicit custom type name via `#[column(type = enum("custom_name"))]`.
#[driver_test(id(ID))]
pub async fn native_enum_custom_type_name(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[column(type = enum("task_priority"))]
    enum Priority {
        Low,
        Medium,
        High,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: ID,
        priority: Priority,
    }

    let mut db = t.setup_db(models!(Task, Priority)).await;

    let task = toasty::create!(Task {
        priority: Priority::Medium
    })
    .exec(&mut db)
    .await?;

    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.priority,
        Priority::Medium
    );

    // Update roundtrip
    let mut task = task;
    task.update().priority(Priority::High).exec(&mut db).await?;
    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.priority,
        Priority::High
    );

    Ok(())
}
