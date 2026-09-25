use crate::prelude::*;

#[driver_test]
pub async fn compare_unindexed_reference_before_schema(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Details {
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        id: String,
        serial: String,
        details: Option<Details>,
    }

    #[derive(Debug, toasty::Model)]
    struct Gadget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        bot_serial: String,
        #[belongs_to(key = bot_serial, references = serial)]
        bot: toasty::Deferred<Bot>,
    }

    let bot = Bot {
        id: "primary".into(),
        serial: "serial".into(),
        details: Some(Details {
            notes: toasty::Deferred::default(),
        }),
    };
    let query = Gadget::filter(Gadget::fields().bot().eq(&bot));
    let primary_query = Bot::filter(Bot::fields().eq(bot));

    let mut db = test.setup_db(models!(Bot, Gadget)).await;
    toasty::create!(Bot {
        id: "primary",
        serial: "serial"
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Gadget {
        bot_serial: "serial"
    })
    .exec(&mut db)
    .await?;

    let found: Vec<Gadget> = query.exec(&mut db).await?;
    assert_struct!(found, [{ bot_serial: "serial" }]);
    let found: Vec<Bot> = primary_query.exec(&mut db).await?;
    assert_struct!(found, [{ id: "primary" }]);

    Ok(())
}

#[driver_test]
pub async fn compare_non_primary_reference_model(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        serial: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Gadget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        bot_serial: String,
        #[belongs_to(key = bot_serial, references = serial)]
        bot: toasty::Deferred<Bot>,
    }

    let mut db = test.setup_db(models!(Bot, Gadget)).await;
    let bots = toasty::create!(Bot::[
        { serial: "B-1" },
        { serial: "B-2" },
        { serial: "B-3" },
    ])
    .exec(&mut db)
    .await?;
    for bot in &bots {
        toasty::create!(Gadget {
            bot_serial: &bot.serial
        })
        .exec(&mut db)
        .await?;
    }

    let found: Vec<Gadget> = Gadget::filter(Gadget::fields().bot().eq(&bots[0]))
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ bot_serial: "B-1" }]);

    let found: Vec<Gadget> = Gadget::filter(Gadget::fields().bot().ne(&bots[0]))
        .exec(&mut db)
        .await?;
    assert_eq_unordered!(found.iter().map(|g| g.bot_serial.as_str()), ["B-2", "B-3"]);

    let found: Vec<Gadget> = Gadget::filter(toasty::stmt::Expr::in_list(
        Gadget::fields().bot(),
        [&bots[0], &bots[1]],
    ))
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(found.iter().map(|g| g.bot_serial.as_str()), ["B-1", "B-2"]);

    Ok(())
}

#[driver_test]
pub async fn compare_non_primary_reference_same_key_type(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        id: String,
        #[unique]
        serial: String,
        #[index]
        label: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Gadget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        bot_serial: String,
        #[belongs_to(key = bot_serial, references = serial)]
        bot: toasty::Deferred<Bot>,
    }

    let mut db = test.setup_db(models!(Bot, Gadget)).await;
    let bots = toasty::create!(Bot::[
        { id: "B-2", serial: "B-1", label: "first" },
        { id: "B-1", serial: "B-2", label: "second" },
    ])
    .exec(&mut db)
    .await?;
    for bot in &bots {
        toasty::create!(Gadget {
            bot_serial: &bot.serial
        })
        .exec(&mut db)
        .await?;
    }

    let found: Vec<Gadget> = Gadget::filter(Gadget::fields().bot().eq(&bots[0]))
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ bot_serial: "B-1" }]);

    let mut bot = Bot::get_by_id(&mut db, &bots[0].id).await?;
    assert!(bot.label.is_unloaded());
    bot.serial = "B-2".to_string();

    let found: Vec<Bot> = Bot::filter(Bot::fields().eq(&bot)).exec(&mut db).await?;
    assert_struct!(found, [{ id: "B-2", serial: "B-1" }]);

    let found: Vec<Gadget> = Gadget::filter(Gadget::fields().bot().eq(bot))
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ bot_serial: "B-2" }]);

    Ok(())
}

#[driver_test]
pub async fn compare_non_primary_reference_in_struct(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        serial: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[index]
        serial: String,
        #[belongs_to(key = serial, references = serial)]
        bot: toasty::Deferred<Bot>,
    }

    #[derive(Debug, toasty::Model)]
    struct Gadget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Bot, Gadget)).await;
    let bots = toasty::create!(Bot::[
        { serial: "B-1" },
        { serial: "B-2" },
        { serial: "B-3" },
    ])
    .exec(&mut db)
    .await?;
    for bot in &bots {
        toasty::create!(Gadget {
            owner: Owner {
                serial: bot.serial.clone(),
                bot: toasty::Deferred::default(),
            }
        })
        .exec(&mut db)
        .await?;
    }

    let found: Vec<Gadget> = Gadget::filter(Gadget::fields().owner().bot().eq(&bots[0]))
        .exec(&mut db)
        .await?;
    assert_struct!(found, [{ owner.serial: "B-1" }]);

    let found: Vec<Gadget> = Gadget::filter(toasty::stmt::Expr::in_list(
        Gadget::fields().owner().bot(),
        [&bots[0], &bots[1]],
    ))
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(
        found.iter().map(|g| g.owner.serial.as_str()),
        ["B-1", "B-2"]
    );

    Ok(())
}

#[driver_test]
pub async fn compare_non_primary_reference_in_enum(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        serial: String,
    }

    #[derive(Debug, toasty::Embed)]
    #[index(serial)]
    enum Owner {
        Bot {
            #[shared(serial)]
            serial: String,
            #[belongs_to(key = serial, references = serial)]
            bot: toasty::Deferred<Bot>,
        },
        Other {
            #[shared(serial)]
            serial: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Gadget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Bot, Gadget)).await;
    let bots = toasty::create!(Bot::[
        { serial: "B-1" },
        { serial: "B-2" },
        { serial: "B-3" },
    ])
    .exec(&mut db)
    .await?;
    let mut ids = vec![];
    for bot in &bots {
        let gadget = toasty::create!(Gadget {
            owner: Owner::Bot { bot }
        })
        .exec(&mut db)
        .await?;
        ids.push(gadget.id);
    }
    toasty::create!(Gadget {
        owner: Owner::Other {
            serial: "B-1".to_string()
        }
    })
    .exec(&mut db)
    .await?;

    let found: Vec<Gadget> = Gadget::filter(
        Gadget::fields()
            .owner()
            .bot()
            .matches(|v| v.bot().eq(&bots[0])),
    )
    .exec(&mut db)
    .await?;
    assert_struct!(found, [{ id: == ids[0] }]);

    let found: Vec<Gadget> = Gadget::filter(
        Gadget::fields()
            .owner()
            .bot()
            .matches(|v| toasty::stmt::Expr::in_list(v.bot(), [&bots[0], &bots[1]])),
    )
    .exec(&mut db)
    .await?;
    assert_eq_unordered!(found.iter().map(|g| &g.id), [&ids[0], &ids[1]]);

    Ok(())
}
