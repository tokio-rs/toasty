use toasty_core::schema::app::Schema;

#[test]
fn pair_inference_reports_each_embedding() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(toasty::Embed)]
    struct Owner {
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        primary: Owner,
        secondary: Owner,
    }
    let error = Schema::from_macro(toasty::models!(Parent, Item)).unwrap_err();
    assert!(error.is_invalid_schema());
    let message = error.to_string();
    assert!(message.contains("primary.parent"), "{message}");
    assert!(message.contains("secondary.parent"), "{message}");
    assert!(message.contains("pair ="), "{message}");
}

#[test]
fn two_inverses_cannot_claim_one_embedding() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_many(pair = owner)]
        first: toasty::Deferred<Vec<Item>>,
        #[has_many(pair = owner.parent)]
        second: toasty::Deferred<Vec<Item>>,
    }
    #[derive(toasty::Embed)]
    struct Owner {
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        owner: Owner,
    }
    let error = Schema::from_macro(toasty::models!(Parent, Item)).unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(error.to_string().contains("more than one inverse"));
}

#[test]
fn has_one_without_pair_is_a_schema_error() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_one]
        item: toasty::Deferred<Option<Item>>,
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
    }
    let error = Schema::from_macro(toasty::models!(Parent, Item)).unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(error.to_string().contains("no matching `BelongsTo`"));
}

#[test]
fn pair_cannot_traverse_a_relation() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_many(pair = parent.items)]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        parent_id: uuid::Uuid,
        #[belongs_to(key = parent_id)]
        parent: toasty::Deferred<Parent>,
    }
    let error = Schema::from_macro(toasty::models!(Parent, Item)).unwrap_err();
    assert!(error.is_invalid_schema());
}

#[test]
fn eager_cycles_through_embeds_are_rejected() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_many]
        items: Vec<Item>,
    }
    #[derive(toasty::Embed)]
    struct Owner {
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: Parent,
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        owner: Owner,
    }
    let error = Schema::from_macro(toasty::models!(Parent, Item)).unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(error.to_string().contains("eager relation cycle"));
}

#[test]
fn inverse_embedded_key_requires_an_index_on_every_backend() {
    #[derive(toasty::Model)]
    struct Parent {
        #[key]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(toasty::Embed)]
    enum Owner {
        Parent {
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
    }
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        owner: Owner,
    }
    use toasty_core::driver::Capability;
    for capability in [
        Capability::SQLITE,
        Capability::POSTGRESQL,
        Capability::MYSQL,
        Capability::DYNAMODB,
        Capability::TURSO,
    ] {
        let app = Schema::from_macro(toasty::models!(Parent, Item)).unwrap();
        let error = toasty_core::schema::Builder::new()
            .build(app, &capability)
            .unwrap_err();
        assert!(error.is_invalid_schema());
        assert!(error.to_string().contains("no index"), "{error}");
    }
}
