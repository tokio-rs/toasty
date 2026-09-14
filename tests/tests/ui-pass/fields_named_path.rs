#![allow(dead_code)]

#[derive(Debug, toasty::Model)]
struct Parent {
    #[key]
    id: i64,

    #[unique]
    path: String,

    #[unique]
    from_path: String,

    #[has_many(pair = parent)]
    children: toasty::Deferred<Vec<Child>>,
}

#[derive(Debug, toasty::Model)]
struct Child {
    #[key]
    id: i64,

    path: String,
    from_path: String,

    #[index]
    parent_id: i64,

    #[belongs_to(key = parent_id, references = id)]
    parent: toasty::Deferred<Parent>,
}

#[derive(Debug, toasty::Embed)]
struct Metadata {
    path: String,
    from_path: String,
}

#[derive(Debug, toasty::Embed)]
enum Locator {
    Named {
        path: String,
        from_path: String,
    },
}

#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    id: i64,

    metadata: Metadata,
    locator: Locator,
}

fn main() {
    let _: toasty::stmt::Path<Parent, String> = Parent::fields().path();
    let _: toasty::stmt::Path<Parent, String> = Parent::fields().from_path();
    let _: toasty::stmt::Path<Parent, toasty::stmt::List<String>> =
        Parent::fields().children().path();
    let _: toasty::stmt::Path<Parent, toasty::stmt::List<String>> =
        Parent::fields().children().from_path();
    let _: toasty::stmt::Path<Child, String> = Child::fields().parent().path();
    let _: toasty::stmt::Path<Child, String> = Child::fields().parent().from_path();

    let _ = Parent::filter_by_path("path");
    let _ = Parent::filter_by_from_path("from_path");

    let _: toasty::stmt::Path<Document, String> = Document::fields().metadata().path();
    let _: toasty::stmt::Path<Document, String> = Document::fields().metadata().from_path();
    let _: toasty::stmt::Path<Document, String> = Document::fields().locator().named().path();
    let _: toasty::stmt::Path<Document, String> =
        Document::fields().locator().named().from_path();
}
