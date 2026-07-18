use crate::prelude::*;

#[driver_test(requires(document_collections))]
pub async fn create_load_and_update(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Exchange {
        #[key]
        id: String,
        request: toasty::JsonValue,
        response: Option<toasty::JsonValue>,
    }

    let mut db = test.setup_db(models!(Exchange)).await;
    assert_struct!(
        column_storage_ty(&db, "exchanges", "request"),
        toasty_core::schema::db::Type::Document { binary: true }
    );
    let request = serde_json::json!({
        "action": "create",
        "arguments": { "name": "Alice" },
    });

    let mut exchange = toasty::create!(Exchange {
        id: "request-1",
        request: request.clone(),
    })
    .exec(&mut db)
    .await?;

    assert_eq!(*exchange.request, request);
    assert_none!(exchange.response);

    let response = serde_json::json!([{"id": 1}, null, true]);
    exchange
        .update()
        .response(toasty::JsonValue(response.clone()))
        .exec(&mut db)
        .await?;

    let reloaded = Exchange::get_by_id(&mut db, "request-1").await?;
    assert_eq!(*reloaded.request, request);
    assert_eq!(*reloaded.response.unwrap(), response);

    Ok(())
}

#[driver_test(requires(document_collections))]
pub async fn nested_in_document(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    struct Profile {
        name: String,
        extra: toasty::JsonValue,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[document]
        profile: Profile,
    }

    let mut db = test.setup_db(models!(User)).await;
    let extra = serde_json::json!({
        "role": "admin",
        "features": ["audit", "replay"],
    });

    toasty::create!(User {
        id: "user-1",
        profile: Profile {
            name: "Alice".into(),
            extra: extra.clone().into(),
        },
    })
    .exec(&mut db)
    .await?;

    let user = User::get_by_id(&mut db, "user-1").await?;
    assert_eq!(user.profile.name, "Alice");
    assert_eq!(*user.profile.extra, extra);

    Ok(())
}

#[driver_test(requires(document_collections))]
pub async fn distinguishes_json_null_from_database_null(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct ValueHolder {
        #[key]
        id: String,
        value: Option<toasty::JsonValue>,
    }

    let mut db = test.setup_db(models!(ValueHolder)).await;

    toasty::create!(ValueHolder::[
        { id: "database-null" },
        { id: "json-null", value: toasty::JsonValue::null() },
    ])
    .exec(&mut db)
    .await?;

    let database_null = ValueHolder::get_by_id(&mut db, "database-null").await?;
    assert_none!(database_null.value);

    let json_null = ValueHolder::get_by_id(&mut db, "json-null").await?;
    assert_eq!(*json_null.value.unwrap(), serde_json::Value::Null);

    Ok(())
}

#[driver_test(requires(document_collections))]
pub async fn updates_json_value_to_database_null(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct ValueHolder {
        #[key]
        id: String,
        value: Option<toasty::JsonValue>,
    }

    let mut db = test.setup_db(models!(ValueHolder)).await;
    let mut holder = toasty::create!(ValueHolder {
        id: "json-null",
        value: toasty::JsonValue::null(),
    })
    .exec(&mut db)
    .await?;

    holder.update().value(None).exec(&mut db).await?;

    let reloaded = ValueHolder::get_by_id(&mut db, "json-null").await?;
    assert_none!(reloaded.value);

    Ok(())
}
