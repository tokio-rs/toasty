use toasty::schema::Register;

#[derive(Debug, toasty::Model)]
struct ModelA {
    #[key]
    id: i64,
}

#[derive(Debug, toasty::Model)]
struct ModelB {
    #[key]
    id: i64,
}

#[test]
fn empty_set() {
    let models = toasty::models!();

    assert!(
        models.is_empty(),
        "expected 0 models in model set, got: {:?}", models.len()
    );
}

#[test]
fn single_type() {
    let models = toasty::models!(ModelA);

    assert!(
        models.contains(ModelA::id()),
        "expected ModelA in model set"
    );
}

#[test]
fn multiple_types() {
    let models = toasty::models!(ModelA, ModelB, tests_fixture_user::User);

    assert!(
        models.contains(ModelA::id()),
        "expected ModelA in model set"
    );
    assert!(
        models.contains(ModelB::id()),
        "expected ModelB in model set"
    );
    assert!(
        models.contains(tests_fixture_user::User::id()),
        "expected tests_fixture_user::User in model set"
    );
}

#[test]
fn current_crate() {
    let models = toasty::models!(crate::*);

    assert!(
        models.contains(ModelA::id()),
        "expected ModelA in model set"
    );
    assert!(
        models.contains(ModelB::id()),
        "expected ModelB in model set"
    );
    assert_eq!(
        models.len(),
        2,
        "expected 2 models in model set, got: {:?}", models.len()
    );
}

#[test]
fn single_external_crate() {
    let models = toasty::models!(tests_fixture_user::*);

    assert!(
        models.contains(tests_fixture_user::User::id()),
        "expected tests_fixture_user::User in model set"
    );
    assert_eq!(
        models.len(),
        1,
        "expected 1 model in model set, got: {:?}", models.len()
    );
}

#[test]
fn multiple_external_crates() {
    let models = toasty::models!(tests_fixture_post::*, tests_fixture_user::*);

    assert!(
        models.contains(tests_fixture_post::Post::id()),
        "expected tests_fixture_user::User in model set"
    );
    assert!(
        models.contains(tests_fixture_user::User::id()),
        "expected tests_fixture_user::User in model set"
    );
    assert_eq!(
        models.len(),
        2,
        "expected 2 models in model set, got: {:?}", models.len()
    );
}

#[test]
fn mixed_inputs() {
    let models = toasty::models!(crate::*, tests_fixture_user::*, tests_fixture_post::Post);

    assert!(
        models.contains(ModelA::id()),
        "expected ModelA in model set"
    );
    assert!(
        models.contains(ModelB::id()),
        "expected ModelB in model set"
    );
    assert!(
        models.contains(tests_fixture_post::Post::id()),
        "expected tests_fixture_user::User in model set"
    );
    assert!(
        models.contains(tests_fixture_user::User::id()),
        "expected tests_fixture_user::User in model set"
    );
    assert_eq!(
        models.len(),
        4,
        "expected 4 models in model set, got: {:?}", models.len()
    );
}

#[test]
fn duplicates() {
    let models = toasty::models!(ModelA, ModelA);

    assert_eq!(
        models.len(),
        1,
        "expected 1 model in model set, got: {:?}", models.len()
    );
}

#[test]
fn trailing_comma() {
    let models = toasty::models!(ModelA, ModelB, );

    assert!(
        models.contains(ModelA::id()),
        "expected ModelA in model set"
    );
    assert!(
        models.contains(ModelB::id()),
        "expected ModelB in model set"
    );
    assert_eq!(
        models.len(),
        2,
        "expected 2 models in model set, got: {:?}", models.len()
    );
}
