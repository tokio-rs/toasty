//! Compile-smoke coverage for advanced documented APIs.
//!
//! These tests intentionally avoid runtime DB setup and focus on ensuring that
//! documented method surfaces and type patterns compile.

#![allow(dead_code)]

#[derive(Debug, toasty::Embed)]
struct ContactMeta {
    #[unique]
    email: String,
    #[index]
    country: String,
}

#[derive(Debug, PartialEq, toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Email {
        #[unique]
        address: String,
    },
    #[column(variant = 2)]
    Phone {
        #[index]
        number: String,
    },
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[index]
    group: String,

    #[index]
    profile_id: Option<uuid::Uuid>,

    #[belongs_to(key = profile_id, references = id)]
    profile: toasty::BelongsTo<Option<Profile>>,

    #[has_many]
    todos: toasty::HasMany<Todo>,

    contact: ContactInfo,
    meta: ContactMeta,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}

#[derive(Debug, toasty::Model)]
struct Person {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[index]
    parent_id: Option<uuid::Uuid>,

    #[belongs_to(key = parent_id, references = id)]
    parent: toasty::BelongsTo<Option<Person>>,

    #[has_many(pair = parent)]
    children: toasty::HasMany<Person>,
}

#[derive(Debug, toasty::Model)]
struct BulkTodo {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,
}

#[test]
fn create_many_builder_api_compiles() {
    let _bulk = BulkTodo::create_many()
        .item(BulkTodo::create().title("one"))
        .with_item(|c| c.title("two"));
}

#[test]
fn relation_batch_create_api_compiles() {
    let _user_create = User::create()
        .name("Ann")
        .group("eng")
        .contact(ContactInfo::Email {
            address: "ann@example.com".to_string(),
        })
        .meta(ContactMeta {
            email: "ann-meta@example.com".to_string(),
            country: "US".to_string(),
        })
        .todo(Todo::create().title("first"))
        .with_todos(|many| {
            many.with_item(|c| c.title("second"))
                .with_item(|c| c.title("third"))
                .with_item(|c| c.title("fourth"))
        });
}

#[test]
fn embedded_enum_filter_api_compiles() {
    let _only_email = User::filter(User::fields().contact().is_email());

    let _email_in_group = User::filter(
        User::fields().group().eq("eng").and(
            User::fields()
                .contact()
                .email()
                .matches(|e| e.address().eq("alice@example.com")),
        ),
    );
}

#[test]
fn include_chain_snippets_compile() {
    let _basic = User::all().include(User::fields().todos());

    let _multiple = Todo::all()
        .include(Todo::fields().user())
        .include(Todo::fields().user().todos());

    let _nested = User::all().include(User::fields().todos().user());
}

#[test]
fn self_referential_and_one_way_belongs_to_compile() {
    let _person_create = Person::create()
        .name("child")
        .parent_id(Some(uuid::Uuid::nil()));

    let _with_children = Person::all().include(Person::fields().children());

    let _user_with_profile = User::create()
        .name("Bob")
        .group("ops")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .meta(ContactMeta {
            email: "bob-meta@example.com".to_string(),
            country: "US".to_string(),
        })
        .profile(Profile::create());
}
