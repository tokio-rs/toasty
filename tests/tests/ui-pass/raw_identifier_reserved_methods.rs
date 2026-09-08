#![allow(dead_code)]

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,

    r#count: i64,
    r#all: i64,

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
    item_id: i64,

    #[belongs_to(key = item_id, references = id)]
    item: toasty::Deferred<Item>,
}

fn main() {
    let _ = Item::fields().r#count();
    let _ = Item::fields().r#all();
    let _ = Item::fields()
        .comments()
        .filter(Comment::fields().id().eq(1))
        .order_by(Comment::fields().id().asc());
}
