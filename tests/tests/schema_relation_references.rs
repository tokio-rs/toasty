#![allow(dead_code)]

use toasty::schema::Model;

#[derive(toasty::Model)]
struct Target {
    #[key]
    id: String,
    #[unique]
    serial: String,
    #[unique]
    unused: String,
    #[index]
    label: String,
}

#[derive(toasty::Model)]
struct Source {
    #[key]
    id: String,
    serial: String,
    #[belongs_to(key = serial, references = serial)]
    target: toasty::Deferred<Target>,
    alternate: String,
    #[belongs_to(key = alternate, references = serial)]
    other: toasty::Deferred<Target>,
}

#[derive(toasty::Embed)]
struct Owner {
    serial: String,
    #[belongs_to(key = serial, references = serial)]
    target: toasty::Deferred<Target>,
}

#[derive(toasty::Embed)]
enum EmbeddedOwner {
    Target {
        id: String,
        serial: String,
        #[belongs_to(key = [id, serial], references = [id, serial])]
        target: toasty::Deferred<Target>,
    },
    None,
}

#[derive(toasty::Model)]
struct EmbeddedSource {
    #[key]
    id: String,
    owner: Owner,
    other: EmbeddedOwner,
}

#[test]
fn records_incoming_references_without_duplicates() {
    let schema = toasty::Db::builder()
        .models(toasty::models!(Source))
        .build_app_schema()
        .unwrap();

    assert_eq!(
        schema
            .model(Target::id())
            .as_root_unwrap()
            .referenced_fields,
        [Target::field_name_to_id("serial")],
    );
    assert!(
        schema
            .model(Source::id())
            .as_root_unwrap()
            .referenced_fields
            .is_empty()
    );
}

#[test]
fn includes_struct_and_enum_foreign_keys() {
    let schema = toasty::Db::builder()
        .models(toasty::models!(EmbeddedSource))
        .build_app_schema()
        .unwrap();

    assert_eq!(
        schema
            .model(Target::id())
            .as_root_unwrap()
            .referenced_fields,
        [
            Target::field_name_to_id("id"),
            Target::field_name_to_id("serial")
        ],
    );
}

#[test]
fn references_belong_to_each_schema() {
    let schema = toasty::Db::builder()
        .models(toasty::models!(Source))
        .build_app_schema()
        .unwrap();
    let target = schema.model(Target::id()).clone();
    let schema = toasty::schema::app::Schema::from_macro([target]).unwrap();

    assert!(
        schema
            .model(Target::id())
            .as_root_unwrap()
            .referenced_fields
            .is_empty()
    );
}
