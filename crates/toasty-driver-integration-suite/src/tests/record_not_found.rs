use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn not_found(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let err = User::all().get(&mut db).await.unwrap_err();
    assert!(
        err.is_record_not_found() | err.is_unsupported_feature(),
        "expected RecordNotFound or UnsupportedFeature, got: {err}"
    );

    // Create and immediately delete to get a known-missing ID
    let user = toasty::create!(User { name: "hello" })
        .exec(&mut db)
        .await?;
    let gone_id = user.id;
    user.delete().exec(&mut db).await?;

    let err = User::filter_by_id(gone_id).get(&mut db).await.unwrap_err();
    assert!(
        err.is_record_not_found(),
        "expected RecordNotFound, got: {err}"
    );

    let err = User::get_by_id(&mut db, gone_id).await.unwrap_err();
    assert!(
        err.is_record_not_found(),
        "expected RecordNotFound, got: {err}"
    );
    Ok(())
}
