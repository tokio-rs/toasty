use toasty::schema::{
    self, BelongsToField, CreateMeta, HasManyField, HasOneField, Load, Model, ModelSet, Register,
    Relation,
};
use toasty::stmt::{Expr, Insert, IntoExpr, IntoInsert, Path};
use toasty::{BelongsTo, Deferred, HasMany, HasOne};
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

impl Register for Dummy {
    fn id() -> schema::app::ModelId {
        schema::app::ModelId(usize::MAX)
    }

    fn schema() -> schema::app::Model {
        panic!("not needed for relation lazy-slot decode tests")
    }

    fn register(_model_set: &mut ModelSet) {}
}

impl Model for Dummy {
    type Query = ();
    type Create = DummyCreate;
    type Update<'a> = ();
    type UpdateQuery = ();
    type Path<Origin> = Path<Origin, Self>;
    type PrimaryKey = i64;

    const CREATE_META: CreateMeta = CreateMeta {
        fields: &[],
        model_name: "Dummy",
    };

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn find_by_primary_key(_id: Expr<Self::PrimaryKey>) -> Self::Query {}
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

impl Relation for Dummy {
    type Model = Dummy;
    type Expr = Dummy;
    type Query = ();
    type Create = DummyCreate;
    type Many = ();
    type ManyField<Origin> = ();
    type One = ();
    type OneField<Origin> = ();
    type OptionOne = ();

    fn new_many_field<Origin>(_path: Path<Origin, toasty::stmt::List<Self::Model>>) {}

    fn field_name_to_id(_name: &str) -> schema::app::FieldId {
        panic!("not needed for relation lazy-slot decode tests")
    }
}

fn dummy_record(id: i64) -> Value {
    Value::record_from_vec(vec![Value::I64(id)])
}

fn loaded_slot(value: Value) -> Value {
    Value::record_from_vec(vec![value])
}

fn assert_has_many_field<F: HasManyField<Target = Dummy>>() {}

fn assert_has_one_field<F: HasOneField<Target = Target>, Target: Relation>() {}

fn assert_belongs_to_field<F: BelongsToField<Target = Target>, Target: Relation>() {}

#[test]
fn direct_and_deferred_relation_field_shapes_are_supported() {
    assert_has_many_field::<HasMany<Dummy>>();
    assert_has_many_field::<Deferred<Vec<Dummy>>>();
    assert_has_many_field::<Vec<Dummy>>();

    assert_has_one_field::<HasOne<Dummy>, Dummy>();
    assert_has_one_field::<HasOne<Option<Dummy>>, Option<Dummy>>();
    assert_has_one_field::<Deferred<Dummy>, Dummy>();
    assert_has_one_field::<Deferred<Option<Dummy>>, Option<Dummy>>();
    assert_has_one_field::<Dummy, Dummy>();
    assert_has_one_field::<Option<Dummy>, Option<Dummy>>();

    assert_belongs_to_field::<BelongsTo<Dummy>, Dummy>();
    assert_belongs_to_field::<BelongsTo<Option<Dummy>>, Option<Dummy>>();
    assert_belongs_to_field::<Deferred<Dummy>, Dummy>();
    assert_belongs_to_field::<Deferred<Option<Dummy>>, Option<Dummy>>();
    assert_belongs_to_field::<Dummy, Dummy>();
    assert_belongs_to_field::<Option<Dummy>, Option<Dummy>>();
}

#[test]
fn has_many_loads_current_direct_list_shape() {
    let relation =
        <HasMany<Dummy> as Load>::load(Value::List(vec![dummy_record(1), dummy_record(2)]))
            .unwrap();

    assert_eq!(
        relation
            .get()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn has_many_loads_lazy_slot_shape() {
    let relation = <HasMany<Dummy> as Load>::load(loaded_slot(Value::List(vec![
        dummy_record(1),
        dummy_record(2),
    ])))
    .unwrap();

    assert_eq!(
        relation
            .get()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn has_one_loads_current_direct_record_shape() {
    let relation = <HasOne<Dummy> as Load>::load(dummy_record(1)).unwrap();

    assert_eq!(relation.get(), &Dummy { id: 1 });
}

#[test]
fn has_one_loads_lazy_slot_shape() {
    let relation = <HasOne<Dummy> as Load>::load(loaded_slot(dummy_record(1))).unwrap();

    assert_eq!(relation.get(), &Dummy { id: 1 });
}

#[test]
fn has_one_option_loads_lazy_slot_null_shape() {
    let relation = <HasOne<Option<Dummy>> as Load>::load(loaded_slot(Value::Null)).unwrap();

    assert_eq!(relation.get(), &None);
}

#[test]
fn belongs_to_loads_current_direct_record_shape() {
    let relation = <BelongsTo<Dummy> as Load>::load(dummy_record(1)).unwrap();

    assert_eq!(relation.get(), &Dummy { id: 1 });
}

#[test]
fn belongs_to_loads_lazy_slot_shape() {
    let relation = <BelongsTo<Dummy> as Load>::load(loaded_slot(dummy_record(1))).unwrap();

    assert_eq!(relation.get(), &Dummy { id: 1 });
}

#[test]
fn belongs_to_option_loads_lazy_slot_null_shape() {
    let relation = <BelongsTo<Option<Dummy>> as Load>::load(loaded_slot(Value::Null)).unwrap();

    assert_eq!(relation.get(), &None);
}
