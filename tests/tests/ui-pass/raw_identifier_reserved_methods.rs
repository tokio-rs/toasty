#![allow(dead_code)]

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,

    r#count: i64,
    r#all: i64,

    #[has_many(pair = item)]
    r#filter: toasty::Deferred<Vec<Comment>>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    id: i64,

    #[index]
    item_id: i64,

    #[belongs_to(key = item_id, references = id)]
    item: toasty::Deferred<Item>,
}

fn main() {
    let _ = Item::fields().r#count();
    let _ = Item::fields().r#all();
    let _ = Item::fields().r#filter();
    let _ = Item::fields()
        .r#filter()
        .filter(Comment::fields().id().eq(1));
}
