use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn create_filter_update_by_raw_identifier_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        r#type: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    let mut user = toasty::create!(User { r#type: "admin" })
        .exec(&mut db)
        .await?;
    assert_eq!(user.r#type, "admin");

    let reload = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(reload.r#type, "admin");

    let by_type = User::filter_by_type("admin").exec(&mut db).await?;
    assert_eq!(by_type.len(), 1);
    assert_eq!(by_type[0].id, user.id);

    user.update().r#type("guest").exec(&mut db).await?;
    assert_eq!(user.r#type, "guest");

    let reload = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(reload.r#type, "guest");

    Ok(())
}
