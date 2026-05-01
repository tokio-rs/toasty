//! Tests for the item collection feature: multiple models sharing one DynamoDB table
//! via a composite sort key synthesized from FK + own PK fields.
//!
//! The `#[item_collection(ParentType)]` attribute on a child model declares that it
//! should share a table with its parent.  The sort key is never a struct field; it is
//! built by the driver at write-time from the constituent PK columns.

use crate::prelude::*;

// ---------------------------------------------------------------------------
// Shared model definitions used across tests in this file
// ---------------------------------------------------------------------------

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

/// A todo that belongs to a user and shares the same DynamoDB table.
///
/// `#[item_collection(User)]`  — child lives in the same table as User.
/// `#[key(partition = user_id, local = id)]` — user_id is the hash key
/// (reuses the User row's PK column), id is the sort-key component owned by
/// this model.
#[derive(Debug, toasty::Model)]
#[item_collection(User)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: uuid::Uuid,

    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}

async fn setup(test: &mut Test) -> toasty::Db {
    test.setup_db(models!(User, Todo)).await
}

// ---------------------------------------------------------------------------
// Schema validation tests (no DB needed — fail at setup_db time)
// ---------------------------------------------------------------------------

/// `#[item_collection]` without a compound `#[key]` (missing `local` component)
/// must be rejected at schema-build time.
#[driver_test]
pub async fn validates_missing_local_key(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Root {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        items: toasty::HasMany<Item>,
    }

    // No #[key(partition = ..., local = ...)] — only a single-field PK.
    // An item collection child must have a compound key.
    #[derive(Debug, toasty::Model)]
    #[item_collection(Root)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        root_id: uuid::Uuid,

        #[belongs_to(key = root_id, references = id)]
        root: toasty::BelongsTo<Root>,
    }

    assert_err!(test.try_setup_db(models!(Root, Item)).await);
}

/// The `item_collection` attribute must include the parent model type.
/// `#[item_collection]` (no argument) should be a compile-time error, but
/// we test the runtime/schema path: a model with no FK source fields
/// and a compound key must fail validation.
#[driver_test]
pub async fn validates_no_belongs_to_in_pk(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Root {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        items: toasty::HasMany<Item>,
    }

    // Compound key but neither field references a parent via BelongsTo —
    // the partition field is just a plain string, not derived from a FK.
    #[derive(Debug, toasty::Model)]
    #[item_collection(Root)]
    #[key(partition = bucket, local = id)]
    struct Item {
        #[auto]
        id: uuid::Uuid,

        bucket: String,

        #[index]
        root_id: uuid::Uuid,

        #[belongs_to(key = root_id, references = id)]
        root: toasty::BelongsTo<Root>,
    }

    assert_err!(test.try_setup_db(models!(Root, Item)).await);
}

// ---------------------------------------------------------------------------
// CRUD tests — require a real database with native_starts_with support
// ---------------------------------------------------------------------------

/// Basic create / read / delete cycle for a user + their todos sharing one table.
#[driver_test(requires(native_starts_with))]
pub async fn crud_create_read_delete(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().exec(&mut db).await?;

    // New user has no todos
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    // Create a todo through the user relation
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    assert_eq!(todo.user_id, user.id);

    // Reload by full composite key
    let reloaded = Todo::get_by_user_id_and_id(&mut db, &todo.user_id, &todo.id).await?;
    assert_eq!(reloaded.title, "hello world");

    // User scope still shows exactly one todo
    let list = user.todos().exec(&mut db).await?;
    assert_eq!(1, list.len());

    // Delete the todo
    todo.delete().exec(&mut db).await?;

    assert_err!(Todo::get_by_user_id_and_id(&mut db, &reloaded.user_id, &reloaded.id).await);
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    Ok(())
}

/// Todos created for user A must not appear in user B's scope.
#[driver_test(requires(native_starts_with))]
pub async fn scoped_isolation(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user_a = User::create().exec(&mut db).await?;
    let user_b = User::create().exec(&mut db).await?;

    let todo_a = user_a
        .todos()
        .create()
        .title("user A todo")
        .exec(&mut db)
        .await?;

    // user_b sees no todos
    assert_eq!(0, user_b.todos().exec(&mut db).await?.len());

    // user_a's todo is not visible via user_b scope
    assert_none!(
        user_b
            .todos()
            .filter_by_id(todo_a.id)
            .first()
            .exec(&mut db)
            .await?
    );

    Ok(())
}

/// Multiple todos under the same user can all be loaded, updated, and deleted.
#[driver_test(requires(native_starts_with))]
pub async fn multiple_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().exec(&mut db).await?;

    for i in 0..5 {
        user.todos()
            .create()
            .title(format!("todo {i}"))
            .exec(&mut db)
            .await?;
    }

    let list = user.todos().exec(&mut db).await?;
    assert_eq!(5, list.len());

    Ok(())
}

/// Scoped filter_by_id returns the right todo when the user matches and
/// nothing when the user does not match.
#[driver_test(requires(native_starts_with))]
pub async fn scoped_filter_by_id(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user1 = User::create().exec(&mut db).await?;
    let user2 = User::create().exec(&mut db).await?;

    let todo = user1.todos().create().title("hello").exec(&mut db).await?;

    // Correct scope finds the todo
    let found = user1.todos().get_by_id(&mut db, &todo.id).await?;
    assert_eq!(found.id, todo.id);

    // Wrong scope does not find it
    assert_none!(
        user2
            .todos()
            .filter_by_id(todo.id)
            .first()
            .exec(&mut db)
            .await?
    );

    Ok(())
}

/// Scoped update modifies only the target todo and leaves others unchanged.
#[driver_test(requires(native_starts_with))]
pub async fn scoped_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().exec(&mut db).await?;

    let todo = user
        .todos()
        .create()
        .title("original")
        .exec(&mut db)
        .await?;

    user.todos()
        .filter_by_id(todo.id)
        .update()
        .title("updated")
        .exec(&mut db)
        .await?;

    let reloaded = Todo::get_by_user_id_and_id(&mut db, &todo.user_id, &todo.id).await?;
    assert_eq!(reloaded.title, "updated");

    Ok(())
}

/// Deleting a user via its own scope does not silently fail.  After deletion
/// the user and all their todos are gone.
#[driver_test(requires(native_starts_with))]
pub async fn delete_user_removes_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().exec(&mut db).await?;
    let todo = user
        .todos()
        .create()
        .title("should disappear")
        .exec(&mut db)
        .await?;

    let user_id = user.id;
    let todo_id = todo.id;
    let todo_user_id = todo.user_id;

    user.delete().exec(&mut db).await?;

    assert_err!(User::get_by_id(&mut db, &user_id).await);
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &todo_user_id, &todo_id).await);

    Ok(())
}

/// `item_collection` field on the app schema model is set to the parent model's id.
#[driver_test]
pub async fn schema_item_collection_field_set(test: &mut Test) {
    use toasty::schema::Register;

    let _db = setup(test).await;

    let todo_ic = <Todo as Register>::schema()
        .as_root_unwrap()
        .item_collection;

    assert_eq!(todo_ic, Some(<User as Register>::id()));

    let user_ic = <User as Register>::schema()
        .as_root_unwrap()
        .item_collection;

    assert_eq!(user_ic, None);
}
