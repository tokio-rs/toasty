use crate::prelude::*;

use toasty::schema::Model;
use toasty::stmt::IntoExpr;

#[driver_test(id(ID))]
pub async fn single_column_pk(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Widget {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(Widget)).await;

    let widget = toasty::create!(Widget { name: "alpha" })
        .exec(&mut db)
        .await?;

    let found = Widget::find_by_primary_key(widget.id.into_expr())
        .get(&mut db)
        .await?;
    assert_eq!(found.name, "alpha");

    Ok(())
}

#[driver_test]
pub async fn composite_pk(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(one, two)]
    struct Pair {
        one: String,
        two: String,
    }

    let mut db = test.setup_db(models!(Pair)).await;

    toasty::create!(Pair::[
        { one: "hello", two: "world" },
        { one: "left",  two: "right" },
    ])
    .exec(&mut db)
    .await?;

    let found = Pair::find_by_primary_key(("hello".to_string(), "world".to_string()).into_expr())
        .get(&mut db)
        .await?;
    assert_eq!(found.one, "hello");
    assert_eq!(found.two, "world");

    Ok(())
}

/// Calling `find_by_primary_key` through a generic bound on
/// `Model<PrimaryKey = ...>` — the use case the trait method exists to enable.
#[driver_test]
pub async fn generic_by_primary_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Widget {
        #[key]
        id: String,

        name: String,
    }

    let mut db = test.setup_db(models!(Widget)).await;

    toasty::create!(Widget {
        id: "w1",
        name: "alpha"
    })
    .exec(&mut db)
    .await?;

    async fn fetch<M>(db: &mut toasty::Db, id: String) -> Result<Vec<M>>
    where
        M: toasty::schema::Model<PrimaryKey = String>,
        M::Query: toasty::stmt::IntoStatement<Returning = toasty::stmt::List<M>>,
    {
        use toasty::stmt::IntoStatement;
        let stmt = M::find_by_primary_key(id.into_expr()).into_statement();
        let executor: &mut dyn toasty::Executor = db;
        executor.exec(stmt).await
    }

    let widgets: Vec<Widget> = fetch(&mut db, "w1".to_string()).await?;
    assert_eq!(widgets.len(), 1);
    assert_eq!(widgets[0].name, "alpha");

    Ok(())
}
