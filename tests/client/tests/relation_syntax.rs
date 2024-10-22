use tests_client::*;

async fn field_with_two_relation_attrs_is_err(_s: impl Setup) {
    toasty_core::schema::from_str(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[relation(references = id)]
            #[relation(references = id)]
            user: User,
        }
        ",
    )
    .unwrap();
}

tests!(
    #[should_panic(expected = "field has more than one relation attribute")]
    field_with_two_relation_attrs_is_err,
);
