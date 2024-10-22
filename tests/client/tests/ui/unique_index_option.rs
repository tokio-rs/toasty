use tests_client::*;

fn main() {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: id;

            #[unique]
            email: Option<string>;
        }"
    );

    // Can't find by none email
    db::User::find_by_email(&None);
}