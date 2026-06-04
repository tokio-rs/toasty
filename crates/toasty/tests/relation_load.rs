use toasty::Deferred;
use toasty::schema::{self, Load, Model, ModelSet, RelationManyField, RelationOneField};
use toasty::stmt::{Expr, Insert, IntoExpr, IntoInsert, Path};
use toasty_core::stmt::{self, Value};

#[derive(Debug, PartialEq)]
struct Dummy {
    id: i64,
}

#[derive(Default)]
struct DummyCreate;

impl Load for Dummy {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Model(Self::id())
    }

    fn load(value: Value) -> toasty::Result<Self> {
        let Value::Record(mut record) = value else {
            return Err(toasty_core::Error::type_conversion(value, "Dummy"));
        };

        let id = record[0].take();
        let Value::I64(id) = id else {
            return Err(toasty_core::Error::type_conversion(id, "Dummy::id"));
        };

        Ok(Self { id })
    }
}

impl Model for Dummy {
    type Query<T> = ();
    type Create = DummyCreate;
    type Update<'a> = ();
    type UpdateQuery = ();
    type Path<Origin> = Path<Origin, Self>;
    type PrimaryKey = i64;
    type ManyField<Origin> = ();
    type OneField<Origin> = ();

    fn id() -> schema::app::ModelId {
        schema::app::ModelId(usize::MAX)
    }

    fn schema() -> schema::app::Model {
        panic!("not needed for relation lazy-slot decode tests")
    }

    fn register(_model_set: &mut ModelSet) {}

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_many_field<Origin>(_path: Path<Origin, toasty::stmt::List<Self>>) {}

    fn field_name_to_id(_name: &str) -> schema::app::FieldId {
        panic!("not needed for relation lazy-slot decode tests")
    }

    fn find_by_primary_key(_id: Expr<Self::PrimaryKey>) -> Self::Query<toasty::stmt::List<Self>> {}

    fn wrap_query<T>(_stmt: toasty::stmt::Query<T>) -> Self::Query<T> {}

    fn query_one(_query: Self::Query<toasty::stmt::List<Self>>) -> Self::Query<Self> {}

    fn query_first(_query: Self::Query<toasty::stmt::List<Self>>) -> Self::Query<Option<Self>> {}
}

impl IntoInsert for DummyCreate {
    type Model = Dummy;

    fn into_insert(self) -> Insert<Self::Model> {
        panic!("not needed for relation lazy-slot decode tests")
    }
}

impl IntoExpr<Dummy> for DummyCreate {
    fn into_expr(self) -> Expr<Dummy> {
        panic!("not needed for relation lazy-slot decode tests")
    }

    fn by_ref(&self) -> Expr<Dummy> {
        panic!("not needed for relation lazy-slot decode tests")
    }
}

fn assert_has_many_field<F: RelationManyField<Target = Dummy>>() {}
fn assert_has_one_field<F: RelationOneField<Target = Dummy>>() {}
fn assert_belongs_to_field<F: RelationOneField<Target = Dummy>>() {}

#[test]
fn deferred_relation_field_shapes_are_supported() {
    assert_has_many_field::<Vec<Dummy>>();
    assert_has_many_field::<Deferred<Vec<Dummy>>>();

    assert_has_one_field::<Dummy>();
    assert_has_one_field::<Option<Dummy>>();
    assert_has_one_field::<Deferred<Dummy>>();
    assert_has_one_field::<Deferred<Option<Dummy>>>();

    assert_belongs_to_field::<Dummy>();
    assert_belongs_to_field::<Option<Dummy>>();
    assert_belongs_to_field::<Deferred<Dummy>>();
    assert_belongs_to_field::<Deferred<Option<Dummy>>>();
}
