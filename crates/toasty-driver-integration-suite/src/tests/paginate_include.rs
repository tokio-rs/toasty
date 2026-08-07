use crate::prelude::*;
use toasty::stmt::Page;

#[driver_test(requires(sql))]
pub async fn include_preserves_pagination_cursor(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Author {
        #[key]
        id: i64,
    }

    #[derive(Debug, toasty::Model)]
    struct Book {
        #[key]
        id: i64,
        shelf: i64,

        author_id: Option<i64>,
        #[belongs_to]
        author: toasty::Deferred<Option<Author>>,
    }

    let mut db = test.setup_db(models!(Author, Book)).await;
    let author = toasty::create!(Author { id: 1 }).exec(&mut db).await?;
    toasty::create!(Book::[
        { id: 1, shelf: 1, author: &author },
        { id: 2, shelf: 1, author: &author },
        { id: 3, shelf: 1, author: &author },
    ])
    .exec(&mut db)
    .await?;

    let first: Page<Book> = Book::all()
        .include(Book::fields().author())
        .order_by((Book::fields().shelf().asc(), Book::fields().id().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;

    assert_struct!(first.items, [_ { id: 1, .. }, _ { id: 2, .. }]);
    assert_struct!(first.items[0].author.get(), Some(_ { id: 1 }));
    assert!(first.has_next());

    let second = first.next(&mut db).await?.unwrap();
    assert_struct!(second.items, [_ { id: 3, .. }]);

    let first_again = second.prev(&mut db).await?.unwrap();
    assert_struct!(first_again.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    Ok(())
}
