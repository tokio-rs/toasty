//! `Path` field-metadata accessors: app name, nullability, and single-field
//! uniqueness — including nested embedded, enum-variant, and relation
//! projections.

use toasty::schema::{Embed, Model};

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

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
#[unique(name)]
enum Creature {
    Human {
        #[shared(name)]
        full_name: String,
    },
    Animal {
        #[shared(name)]
        nickname: String,
    },
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Character {
    #[key]
    id: i64,
    creature: Creature,
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
struct ApiKey {
    #[key]
    id: i64,
    #[unique]
    token: Option<String>,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Customer {
    #[key]
    id: i64,
    address: Option<MailAddress>,
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

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Article {
    #[key]
    id: i64,
    #[has_many]
    tags: toasty::Deferred<Vec<Tag>>,
    #[has_many(via = tags.name)]
    tag_names: Vec<String>,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Tag {
    #[key]
    id: i64,
    #[index]
    article_id: i64,
    #[belongs_to(key = article_id, references = id)]
    article: toasty::Deferred<Article>,
    name: String,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct DocAddress {
    city: String,
    zip: Option<String>,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct DocProfile {
    // App-level unique index with no database backing: `collect_indices`
    // only recurses into column-expanded embeds, so `is_unique()` must
    // report `false` through a `#[document]` traversal.
    #[unique]
    name: String,
    nickname: Option<String>,
    #[document]
    address: DocAddress,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct DocAccount {
    #[key]
    id: i64,
    #[document]
    profile: DocProfile,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Tagged {
    #[key]
    id: i64,
    tags: Vec<String>,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct Wrapper(String);

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct NewtypeHolder {
    #[key]
    id: i64,
    wrapper: Wrapper,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct NewtypeId {
    #[key]
    id: Wrapper,
    #[unique]
    token: Wrapper,
    plain: Wrapper,
}

#[derive(Debug, toasty::Embed)]
#[allow(dead_code)]
struct OuterWrapper(Wrapper);

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct NestedNewtypeId {
    #[key]
    id: i64,
    #[unique]
    outer: OuterWrapper,
}

#[test]
fn field_metadata_single_path() {
    let email = User::fields().email();
    assert_eq!(email.field_name().as_deref(), Some("email"));
    assert!(!email.is_nullable());
    assert!(email.is_unique());

    let name = User::fields().name();
    assert_eq!(name.field_name().as_deref(), Some("name"));
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
    assert_eq!(city.field_name().as_deref(), Some("city"));
    assert!(!city.is_nullable());
    assert!(!city.is_unique());

    let nickname = User::fields().profile().nickname();
    assert_eq!(nickname.field_name().as_deref(), Some("nickname"));
    assert!(nickname.is_nullable());
}

#[test]
fn field_metadata_variant_path() {
    let address = User::fields().contact().email().address();
    assert_eq!(address.field_name().as_deref(), Some("address"));
    assert!(!address.is_nullable());
    // Index membership only: rows of every other variant store `NULL` here,
    // which unique indices leave non-conflicting, so this is not cursor-safe.
    assert!(address.is_unique());

    let country_code = User::fields().contact().phone().country_code();
    assert_eq!(country_code.field_name().as_deref(), Some("country_code"));
    assert!(!country_code.is_nullable());
    assert!(!country_code.is_unique());

    let number = User::fields().contact().phone().number();
    assert_eq!(number.field_name().as_deref(), Some("number"));
    assert!(number.is_nullable());
}

#[test]
fn field_metadata_variant_nested_embed_path() {
    let street = User::fields().contact().post().mail().street();
    assert_eq!(street.field_name().as_deref(), Some("street"));
    assert!(!street.is_nullable());
    // Index membership only: same inactive-variant `NULL` caveat as above.
    assert!(street.is_unique());

    let po_box = User::fields().contact().post().mail().po_box();
    assert_eq!(po_box.field_name().as_deref(), Some("po_box"));
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
    assert_eq!(username.field_name().as_deref(), Some("username"));
    assert!(!username.is_nullable());
}

#[test]
fn field_metadata_document_path() {
    let name = DocAccount::fields().profile().name();
    assert_eq!(name.field_name().as_deref(), Some("name"));
    assert!(!name.is_nullable());
    // `DocProfile::name` carries `#[unique]` at the app level, but the
    // traversal crosses `#[document]` storage (no DB index), so `false`.
    assert!(!name.is_unique());

    let nickname = DocAccount::fields().profile().nickname();
    assert_eq!(nickname.field_name().as_deref(), Some("nickname"));
    assert!(nickname.is_nullable());

    // Nested `#[document]` inside a `#[document]`.
    let city = DocAccount::fields().profile().address().city();
    assert_eq!(city.field_name().as_deref(), Some("city"));
    assert!(!city.is_nullable());

    let zip = DocAccount::fields().profile().address().zip();
    assert_eq!(zip.field_name().as_deref(), Some("zip"));
    assert!(zip.is_nullable());
}

#[test]
fn field_metadata_composite_pk_fields_not_unique() {
    assert!(!Membership::fields().org_id().is_unique());
    assert!(!Membership::fields().user_id().is_unique());
}

#[test]
fn field_metadata_nullable_unique_reports_membership() {
    let token = ApiKey::fields().token();
    assert!(token.is_nullable());
    // Index membership only: `NULL`s do not conflict, so `NULL`
    // tokens can repeat despite `is_unique()`.
    assert!(token.is_unique());
}

#[test]
fn field_metadata_nullable_parent_embed_reports_membership() {
    // Typed `fields()` chaining stops at `Option<MailAddress>` (plain `Path`,
    // no sub-field methods), so build the projection with `chain`.
    let address =
        Customer::path_field::<Option<MailAddress>>(Customer::field_name_to_id("address").index);
    // `street` is field 0 of `MailAddress`.
    let street = address.chain(MailAddress::path_field::<String>(0));
    assert_eq!(street.field_name().as_deref(), Some("street"));
    assert!(!street.is_nullable());
    // Index membership only: `None` parents store `NULL` here, which unique
    // indices leave non-conflicting, so this is not cursor-safe.
    assert!(street.is_unique());
}

#[test]
fn field_metadata_shared_unique() {
    // `#[unique(name)]` stores the first `#[shared(name)]` member only, but
    // constrains the shared column for every member.
    let human = Character::fields().creature().human().full_name();
    assert_eq!(human.field_name().as_deref(), Some("full_name"));
    assert!(!human.is_nullable());
    assert!(human.is_unique());

    let animal = Character::fields().creature().animal().nickname();
    assert_eq!(animal.field_name().as_deref(), Some("nickname"));
    assert!(!animal.is_nullable());
    assert!(animal.is_unique());
}

#[test]
fn field_metadata_list_path_nullability() {
    // A collection field targets `List<T>`, which is never `Option`-wrapped.
    assert!(!Tagged::fields().tags().is_nullable());

    // An `Option<Vec<T>>` field keeps the `Option` wrapper as its path
    // target (`Field::ExprTarget` is `Self`), so the `U: Field` impl answers,
    // not the `List<U>` one. The derive does not declare that shape, so this
    // is a type-level assertion: `path_field` pairs the target type with an
    // arbitrary index, and only the target type reaches `is_nullable`.
    let notes = Tagged::path_field::<Option<Vec<String>>>(Tagged::field_name_to_id("tags").index);
    assert!(notes.is_nullable());
}

#[test]
fn field_metadata_newtype_inner_nullable_unique() {
    let inner = NewtypeHolder::fields().wrapper().inner();
    assert!(!inner.is_nullable());
    assert!(!inner.is_unique());
}

#[test]
fn field_metadata_newtype_inner_name_is_none() {
    let inner = NewtypeHolder::fields().wrapper().inner();
    assert!(inner.field_name().is_none());
}

#[test]
fn field_metadata_newtype_inner_unique() {
    // A transparent newtype stores its parent field's column, so a unique
    // index on the parent constrains the inner field too.
    let id = NewtypeId::fields().id().inner();
    assert!(id.is_unique());

    let token = NewtypeId::fields().token().inner();
    assert!(token.is_unique());

    let plain = NewtypeId::fields().plain().inner();
    assert!(!plain.is_unique());
}

#[test]
fn field_metadata_nested_newtype_inner_unique() {
    // Every layer of a nested chain maps to the same column.
    let nested = NestedNewtypeId::fields().outer().inner().inner();
    assert!(nested.is_unique());
}

#[test]
#[should_panic(expected = "path does not end at a field")]
fn field_metadata_empty_path_panics() {
    let _ = User::path_root().field_name();
}

#[test]
#[should_panic(expected = "path does not end at a field")]
fn field_metadata_out_of_bounds_panics() {
    // Hand-built paths can name a field index the model does not have.
    let _ = User::path_field::<String>(99).field_name();
}

#[test]
fn field_metadata_relation_path() {
    // A has-many crossing resolves on the target model.
    let title = Author::fields().posts().title();
    assert_eq!(title.field_name().as_deref(), Some("title"));
    assert!(!title.is_unique());

    // A single-field primary key reached through a to-many relation reports
    // the leaf field's index membership, not uniqueness per author.
    assert!(Author::fields().posts().id().is_unique());

    // A belongs-to crossing resolves on the target model too.
    let author_id = Post::fields().author().id();
    assert_eq!(author_id.field_name().as_deref(), Some("id"));

    // A path may end at a relation field.
    let author =
        Post::path_field::<toasty::Deferred<Author>>(Post::field_name_to_id("author").index);
    assert_eq!(author.field_name().as_deref(), Some("author"));
}

#[test]
fn field_metadata_scalar_via_path() {
    // A scalar-terminal via is a leaf: it names the via field itself.
    let names = Article::fields().tag_names();
    assert_eq!(names.field_name().as_deref(), Some("tag_names"));
    assert!(!names.is_unique());
}

#[test]
#[should_panic(expected = "cannot project through non-embedded field")]
fn field_metadata_scalar_via_projection_panics() {
    // The via's terminal is a scalar, so there is no model to project into.
    let names = Article::fields().tag_names();
    let name = Tag::path_field::<String>(Tag::field_name_to_id("name").index);
    let _ = names.chain(name).field_name();
}
