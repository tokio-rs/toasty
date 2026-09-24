#![allow(dead_code)]

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,

    r#count: i64,
    r#all: i64,
    r#is_none: bool,
    r#is_some: bool,

    metadata: Metadata,

    #[has_many(pair = item)]
    comments: toasty::Deferred<Vec<Comment>>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    id: i64,

    r#filter: i64,
    r#order_by: i64,

    #[index]
    item_id: Option<i64>,

    #[belongs_to(key = item_id, references = id)]
    item: toasty::Deferred<Option<Item>>,
}

#[derive(Debug, toasty::Embed)]
struct Metadata {
    is_none: bool,
    is_some: bool,
}

fn main() {
    let _ = Item::fields().r#count();
    let _ = Item::fields().r#all();
    let _: toasty::stmt::Expr<bool> = Comment::fields().item().is_none();
    let _: toasty::stmt::Expr<bool> = Comment::fields().item().is_some();
    let _ = Item::fields().metadata().is_none().eq(true);
    let _ = Item::fields().metadata().is_some().eq(true);
    let _ = Item::fields()
        .comments()
        .filter(Comment::fields().id().eq(1))
        .order_by(Comment::fields().id().asc());
}
