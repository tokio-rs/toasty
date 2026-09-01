//! `Path` field-metadata accessors: app name, nullability, and single-field
//! uniqueness — including nested embedded and enum-variant projections — plus
//! the `CorePath` re-export.

use toasty::schema::Model;
use toasty::stmt::CorePath;

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
#[unique(name, alias)]
struct User {
    #[key]
    id: i64,
    #[unique]
    email: String,
    name: String,
    alias: String,
    bio: Option<String>,
    profile: Profile,
    contact: Contact,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct Profile {
    city: String,
    nickname: Option<String>,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
#[unique(email::address)]
enum Contact {
    Email {
        address: String,
    },
    Phone {
        country_code: String,
        number: Option<String>,
    },
    Post {
        mail: MailAddress,
    },
    Chat {
        handle: Handle,
    },
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct MailAddress {
    #[unique]
    street: String,
    po_box: Option<String>,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
enum Handle {
    Telegram { username: String },
    Signal { id: String },
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Membership {
    #[key]
    org_id: i64,
    #[key]
    user_id: i64,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Author {
    #[key]
    id: i64,
    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Post {
    #[key]
    id: i64,
    #[index]
    author_id: i64,
    #[belongs_to(key = author_id, references = id)]
    author: toasty::Deferred<Author>,
    title: String,
}

#[test]
fn field_metadata_single_path() {
    let email = User::fields().email();
    assert_eq!(email.field_name(), "email");
    assert!(!email.is_nullable());
    assert!(email.is_unique());

    let name = User::fields().name();
    assert_eq!(name.field_name(), "name");
    assert!(!name.is_unique());
    assert!(!name.is_nullable());

    assert!(!User::fields().alias().is_unique());

    let bio = User::fields().bio();
    assert!(bio.is_nullable());
    assert!(!bio.is_unique());

    let id = User::fields().id();
    assert!(id.is_unique());
}

#[test]
fn field_metadata_composite_path() {
    let city = User::fields().profile().city();
    assert_eq!(city.field_name(), "city");
    assert!(!city.is_nullable());
    assert!(!city.is_unique());

    let nickname = User::fields().profile().nickname();
    assert_eq!(nickname.field_name(), "nickname");
    assert!(nickname.is_nullable());
}

#[test]
fn field_metadata_variant_path() {
    let address = User::fields().contact().email().address();
    assert_eq!(address.field_name(), "address");
    assert!(!address.is_nullable());
    assert!(address.is_unique());

    let country_code = User::fields().contact().phone().country_code();
    assert_eq!(country_code.field_name(), "country_code");
    assert!(!country_code.is_nullable());
    assert!(!country_code.is_unique());

    let number = User::fields().contact().phone().number();
    assert_eq!(number.field_name(), "number");
    assert!(number.is_nullable());
}

#[test]
fn field_metadata_variant_nested_embed_path() {
    let street = User::fields().contact().post().mail().street();
    assert_eq!(street.field_name(), "street");
    assert!(!street.is_nullable());
    assert!(street.is_unique());

    let po_box = User::fields().contact().post().mail().po_box();
    assert_eq!(po_box.field_name(), "po_box");
    assert!(po_box.is_nullable());
}

#[test]
fn field_metadata_nested_variant_path() {
    let username = User::fields()
        .contact()
        .chat()
        .handle()
        .telegram()
        .username();
    assert_eq!(username.field_name(), "username");
    assert!(!username.is_nullable());
}

#[test]
fn field_metadata_composite_pk_fields_not_unique() {
    assert!(!Membership::fields().org_id().is_unique());
    assert!(!Membership::fields().user_id().is_unique());
}

#[test]
fn path_converts_to_core_path() {
    let core: CorePath = User::fields().email().into();
    let expected = User::field_name_to_id("email").index;
    assert_eq!(core.projection.as_slice(), &[expected]);
}

#[test]
#[should_panic(expected = "path does not end at a field")]
fn field_metadata_empty_path_panics() {
    let _ = User::path_root().field_name();
}

#[test]
#[should_panic(expected = "cannot project through non-embedded field")]
fn field_metadata_relation_projection_panics() {
    let posts =
        Author::path_field::<toasty::stmt::List<Post>>(Author::field_name_to_id("posts").index);
    let title = Post::path_field::<String>(Post::field_name_to_id("title").index);
    let _ = posts.chain(title).field_name();
}
