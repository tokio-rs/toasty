use tests_client::*;

fn main() {
    toasty_schema::from_str(
        "
        model User {
            #[key]
            #[auto]
            id: id;

            profile: Option<Profile>;
        }

        model Profile {
            #[key]
            #[auto]
            id: id;

            user: User,
        }
        ",
    )
    .unwrap();
}