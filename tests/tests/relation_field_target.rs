use toasty::Deferred;
use toasty::schema::Model;
use toasty::stmt::{Expr, Include, IntoExpr, List, Path};

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: uuid::Uuid,
    name: String,
    #[has_many(pair = deferred)]
    relations: Deferred<Vec<Relations>>,
}

type RequiredUser = User;
type OptionalUser = Option<RequiredUser>;
type DeferredUser = Deferred<RequiredUser>;
type DeferredOptionalUser = Deferred<OptionalUser>;

#[derive(Debug, toasty::Model)]
struct Relations {
    #[key]
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    optional_user_id: Option<uuid::Uuid>,
    #[belongs_to(key = user_id)]
    eager: User,
    #[belongs_to(key = optional_user_id)]
    eager_optional: Option<User>,
    #[belongs_to(key = user_id)]
    deferred: Deferred<User>,
    #[belongs_to(key = optional_user_id)]
    deferred_optional: Deferred<Option<User>>,
    #[belongs_to(key = user_id)]
    eager_alias: RequiredUser,
    #[belongs_to(key = optional_user_id)]
    eager_optional_alias: OptionalUser,
    #[belongs_to(key = user_id)]
    deferred_alias: DeferredUser,
    #[belongs_to(key = optional_user_id)]
    deferred_optional_alias: DeferredOptionalUser,
}

fn required<Origin>(fields: <User as Model>::OneField<Origin, User>) {
    let _: Expr<User> = fields.by_ref();
    let _: Path<Origin, User> = fields.into();
}

fn optional<Origin>(fields: <User as Model>::OneField<Origin, Option<User>>) {
    let _: Expr<Option<User>> = fields.by_ref();
    let _: Path<Origin, Option<User>> = fields.into();
}

#[test]
fn belongs_to_target_types() {
    required::<Relations>(Relations::fields().eager());
    optional::<Relations>(Relations::fields().eager_optional());
    required::<Relations>(Relations::fields().deferred());
    optional::<Relations>(Relations::fields().deferred_optional());
}

#[test]
fn aliases_resolve_target_types() {
    required::<Relations>(Relations::fields().eager_alias());
    optional::<Relations>(Relations::fields().eager_optional_alias());
    required::<Relations>(Relations::fields().deferred_alias());
    optional::<Relations>(Relations::fields().deferred_optional_alias());
}

#[test]
fn has_one_target_types() {
    #[derive(Debug, toasty::Model)]
    struct Account {
        #[key]
        id: uuid::Uuid,
        #[has_one(pair = eager_account)]
        eager: Profile,
        #[has_one(pair = eager_optional_account)]
        eager_optional: Option<Profile>,
        #[has_one(pair = deferred_account)]
        deferred: Deferred<Profile>,
        #[has_one(pair = deferred_optional_account)]
        deferred_optional: Deferred<Option<Profile>>,
        #[has_one(via = eager)]
        optional_via_required: Deferred<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        id: uuid::Uuid,
        #[unique]
        eager_account_id: Option<uuid::Uuid>,
        #[belongs_to(key = eager_account_id)]
        eager_account: Deferred<Option<Account>>,
        #[unique]
        eager_optional_account_id: Option<uuid::Uuid>,
        #[belongs_to(key = eager_optional_account_id)]
        eager_optional_account: Deferred<Option<Account>>,
        #[unique]
        deferred_account_id: Option<uuid::Uuid>,
        #[belongs_to(key = deferred_account_id)]
        deferred_account: Deferred<Option<Account>>,
        #[unique]
        deferred_optional_account_id: Option<uuid::Uuid>,
        #[belongs_to(key = deferred_optional_account_id)]
        deferred_optional_account: Deferred<Option<Account>>,
    }

    let _: <Profile as Model>::OneField<Account, Profile> = Account::fields().eager();
    let _: <Profile as Model>::OneField<Account, Option<Profile>> =
        Account::fields().eager_optional();
    let _: <Profile as Model>::OneField<Account, Profile> = Account::fields().deferred();
    let _: <Profile as Model>::OneField<Account, Option<Profile>> =
        Account::fields().deferred_optional();
    let _: <Profile as Model>::OneField<Account, Option<Profile>> =
        Account::fields().optional_via_required();
}

#[test]
fn has_one_via_target_types() {
    type OptionalRelations = Option<Relations>;
    type DeferredOptionalRelations = Deferred<OptionalRelations>;

    #[derive(Debug, toasty::Model)]
    struct ViaRelations {
        #[key]
        id: uuid::Uuid,
        relations_id: uuid::Uuid,
        optional_relations_id: Option<uuid::Uuid>,
        #[belongs_to(key = relations_id)]
        eager: Relations,
        #[belongs_to(key = optional_relations_id)]
        eager_optional: OptionalRelations,
        #[belongs_to(key = relations_id)]
        deferred: Deferred<Relations>,
        #[belongs_to(key = optional_relations_id)]
        deferred_optional: DeferredOptionalRelations,
        #[has_one(via = eager.eager_alias)]
        required_eager: RequiredUser,
        #[has_one(via = deferred.deferred_alias)]
        required_deferred: DeferredUser,
        #[has_one(via = eager.eager)]
        optional_from_required: OptionalUser,
        #[has_one(via = eager_optional.eager)]
        optional_eager_intermediate: Option<User>,
        #[has_one(via = deferred_optional.deferred)]
        optional_deferred_intermediate: DeferredOptionalUser,
        #[has_one(via = eager.eager_optional_alias)]
        optional_eager_terminal: OptionalUser,
        #[has_one(via = deferred.deferred_optional_alias)]
        optional_deferred_terminal: DeferredOptionalUser,
        #[has_one(via = eager_optional.eager_optional)]
        optional_both: Option<User>,
        #[has_one(via = required_deferred)]
        required_nested: DeferredUser,
        #[has_one(via = optional_deferred_intermediate)]
        optional_nested: DeferredOptionalUser,
    }

    let fields = ViaRelations::fields();
    required::<ViaRelations>(fields.required_eager());
    required::<ViaRelations>(fields.required_deferred());
    optional::<ViaRelations>(fields.optional_from_required());
    optional::<ViaRelations>(fields.optional_eager_intermediate());
    optional::<ViaRelations>(fields.optional_deferred_intermediate());
    optional::<ViaRelations>(fields.optional_eager_terminal());
    optional::<ViaRelations>(fields.optional_deferred_terminal());
    optional::<ViaRelations>(fields.optional_both());
    required::<ViaRelations>(fields.required_nested());
    optional::<ViaRelations>(fields.optional_nested());
}

#[derive(Debug, toasty::Embed)]
struct Attribution {
    user_id: uuid::Uuid,
    optional_user_id: Option<uuid::Uuid>,
    #[belongs_to(key = user_id)]
    required: Deferred<User>,
    #[belongs_to(key = optional_user_id)]
    optional: Deferred<Option<User>>,
    #[belongs_to(key = user_id)]
    required_alias: DeferredUser,
    #[belongs_to(key = optional_user_id)]
    optional_alias: DeferredOptionalUser,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Person {
        user_id: uuid::Uuid,
        optional_user_id: Option<uuid::Uuid>,
        #[belongs_to(key = user_id)]
        required: Deferred<User>,
        #[belongs_to(key = optional_user_id)]
        optional: Deferred<Option<User>>,
        #[belongs_to(key = user_id)]
        required_alias: DeferredUser,
        #[belongs_to(key = optional_user_id)]
        optional_alias: DeferredOptionalUser,
    },
    Nobody,
}

#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    id: uuid::Uuid,
    attribution: Attribution,
    owner: Owner,
}

#[test]
fn embedded_relation_target_types() {
    let fields = Article::fields().attribution();
    required::<Article>(fields.required());
    optional::<Article>(fields.optional());
    required::<Article>(fields.required_alias());
    optional::<Article>(fields.optional_alias());

    let fields = Article::fields().owner().person();
    required::<Article>(fields.required());
    optional::<Article>(fields.optional());
    required::<Article>(fields.required_alias());
    optional::<Article>(fields.optional_alias());
}

#[test]
fn root_fields_and_constructor_preserve_target() {
    let fields: <User as Model>::Path<User> = User::fields();
    required::<User>(fields);
    required::<User>(User::new_root_path());
    required::<User>(
        <User as toasty::codegen_support::ModelCodegen>::new_one_field(User::path_root()),
    );

    let path: Path<Relations, Option<User>> = Relations::fields().deferred_optional().into();
    optional::<Relations>(<User as toasty::codegen_support::ModelCodegen>::new_one_field(path));
    required::<User>(Relations::fields().deferred_optional().into_root());
}

#[test]
fn optional_fields_support_existing_operations() {
    let fields = || Relations::fields().deferred_optional();
    let user = User {
        id: uuid::Uuid::nil(),
        name: "Alice".into(),
        relations: Deferred::default(),
    };

    let _: Path<Relations, String> = fields().name();
    let _: Path<Relations, List<User>> = fields().relations().eager_optional().into();
    let _: Expr<Option<User>> = fields().into_expr();
    let _: Expr<bool> = fields().eq(&user);
    let _: Expr<bool> = fields().ne(user);
    let _: Expr<bool> = fields().eq(Relations::fields().eager_optional());
    let _: Expr<bool> = fields().ne(Relations::fields().eager_optional_alias());
    let _: Expr<bool> = fields().in_query(User::all());
    let _: Include<Relations, Option<User>> = fields().into();
    let _: Include<Relations, Option<User>> = fields().filter(User::fields().name().eq("Alice"));
    let _: Include<Relations, Option<User>> = fields().order_by(User::fields().name().asc());
    let _: <User as Model>::Create = fields().create();

    let _ = Relations::all().include(fields());
    let _ = Relations::all().include(fields().filter(User::fields().name().eq("Alice")));
    let _ = Relations::all().include(fields().order_by(User::fields().name().asc()));
    let _: toasty::stmt::Query<List<Option<User>>> = Relations::all().select(fields());

    let _: Path<Article, String> = Article::fields().attribution().optional().name();
    let _: Path<Article, String> = Article::fields().owner().person().optional().name();
    let _ = Article::all().include(Article::fields().attribution().optional());
    let _ = Article::all().include(Article::fields().owner().person().optional());
}
