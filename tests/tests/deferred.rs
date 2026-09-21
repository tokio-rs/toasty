use std::collections::HashSet;
use toasty::Deferred;

#[test]
fn equality_compares_load_state_and_value() {
    let unloaded = Deferred::<i32>::default();
    let loaded = Deferred::from(42);

    assert_eq!(unloaded, Deferred::default());
    assert_ne!(unloaded, loaded);
    assert_ne!(loaded, unloaded);
    assert_eq!(loaded, Deferred::from(42));
    assert_ne!(loaded, Deferred::from(43));
}

#[test]
fn loaded_none_is_distinct_from_unloaded() {
    let unloaded = Deferred::<Option<i32>>::default();
    let loaded_none = Deferred::from(None);

    assert_ne!(unloaded, loaded_none);
    assert_eq!(loaded_none, Deferred::from(None));
    assert_ne!(loaded_none, Deferred::from(Some(42)));
}

#[test]
fn partial_eq_does_not_require_eq() {
    assert_eq!(Deferred::from(1.5), Deferred::from(1.5));
    assert_ne!(Deferred::from(f64::NAN), Deferred::from(f64::NAN));
}

#[test]
fn equal_values_deduplicate_in_hash_sets() {
    let mut values = HashSet::new();

    for value in [
        Deferred::default(),
        Deferred::from(None),
        Deferred::from(Some(42)),
        Deferred::from(Some(43)),
    ] {
        assert!(values.insert(value.clone()));
        assert!(!values.insert(value));
    }

    assert_eq!(values.len(), 4);
}

#[test]
fn models_with_relations_derive_equality_and_hash() {
    #[derive(Clone, Debug, PartialEq, Eq, Hash, toasty::Model)]
    struct User {
        #[key]
        id: i64,
        name: String,
        #[has_many]
        posts: Deferred<Vec<Post>>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, toasty::Model)]
    struct Post {
        #[key]
        id: i64,
        #[index]
        user_id: i64,
        #[belongs_to(key = user_id, references = id)]
        user: Deferred<User>,
    }

    let user = User {
        id: 1,
        name: "Alice".into(),
        posts: Deferred::default(),
    };
    assert_eq!(user, user.clone());

    let post = Post {
        id: 2,
        user_id: user.id,
        user: Deferred::from(user.clone()),
    };
    assert_eq!(post, post.clone());
    assert!(HashSet::from([post.clone()]).contains(&post));

    let mut changed_post = post.clone();
    let mut changed_user = user.clone();
    changed_user.name = "Bob".into();
    changed_post.user = changed_user.into();
    assert_ne!(post, changed_post);

    let mut loaded_user = user.clone();
    loaded_user.posts = vec![post].into();
    assert_ne!(user, loaded_user);
    assert_eq!(loaded_user, loaded_user.clone());
    assert!(HashSet::from([loaded_user.clone()]).contains(&loaded_user));

    let mut changed_user = loaded_user.clone();
    changed_user.posts = vec![changed_post].into();
    assert_ne!(loaded_user, changed_user);
}
