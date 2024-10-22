use tests_client::*;

async fn crud_user_profile_one_direction(_s: impl Setup) {
    /*
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: id;

            name: string;

            #[relation(references = id)]
            profile: Option<Profile>;
        }

        model Profile {
            #[key]
            #[auto]
            id: id;

            bio: string;
        }
        "
    );
    */
}

tests!(
    #[ignore]
    crud_user_profile_one_direction,
);
