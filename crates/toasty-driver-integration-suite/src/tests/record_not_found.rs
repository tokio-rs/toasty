use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn not_found(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: String,
    }

    let mut db = t.setup_db(toasty::models!(Item)).await;

    let err = Item::all().get(&mut db).await.unwrap_err();
    assert!(
        err.is_record_not_found() | err.is_unsupported_feature(),
        "expected RecordNotFound or UnsupportedFeature, got: {err}"
    );

    // Create and immediately delete to get a known-missing ID
    let item = Item::create().val("hello").exec(&mut db).await?;
    let gone_id = item.id;
    item.delete().exec(&mut db).await?;

    let err = Item::filter_by_id(gone_id).get(&mut db).await.unwrap_err();
    assert!(
        err.is_record_not_found(),
        "expected RecordNotFound, got: {err}"
    );

    let err = Item::get_by_id(&mut db, gone_id).await.unwrap_err();
    assert!(
        err.is_record_not_found(),
        "expected RecordNotFound, got: {err}"
    );
    Ok(())
}
