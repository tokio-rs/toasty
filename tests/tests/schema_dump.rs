//! The schema the standalone CLI reads out of a build artifact must be the
//! same schema the program itself would run against. These tests exercise the
//! dump payload directly; `standalone_cli.rs` covers the CLI end to end.

use toasty::schema::dump::{self, Dump};
use toasty_core::stmt::Value;

#[derive(Debug, toasty::Model)]
struct SchemaDumpUser {
    #[key]
    id: u64,

    #[unique]
    email: String,

    #[has_many(pair = user)]
    orders: toasty::Deferred<Vec<SchemaDumpOrder>>,

    status: SchemaDumpStatus,

    priority: SchemaDumpPriority,
}

#[derive(Debug, toasty::Model)]
struct SchemaDumpOrder {
    #[key]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<SchemaDumpUser>,
}

#[derive(Debug, toasty::Embed)]
enum SchemaDumpStatus {
    Draft,
    Placed { note: String },
}

/// Explicit integer discriminants take the codec's other arm.
#[derive(Debug, toasty::Embed)]
enum SchemaDumpPriority {
    #[column(variant = 1)]
    Low,
    #[column(variant = 2)]
    High,
}

/// The dump payload, decoded the way the CLI decodes it.
fn round_trip() -> Dump {
    let mut out = Vec::new();
    dump::write_dump(&mut out).unwrap();

    let text = String::from_utf8(out).unwrap();
    let json = text
        .split_once(dump::DUMP_MARKER)
        .expect("payload is preceded by the marker")
        .1;

    serde_json::from_str(json.trim()).unwrap()
}

/// The whole point of running the user's own artifact is that relations,
/// embeds, and enum variants come back linked, not as isolated declarations.
#[test]
fn dump_round_trips_a_linked_schema() {
    let dumped = round_trip().schema;
    let built = toasty::Db::builder()
        .models(toasty::models!(SchemaDumpUser, SchemaDumpOrder))
        .build_app_schema()
        .unwrap();

    for model in built.models() {
        let name = model.name().upper_camel_case();
        let found = dumped
            .models()
            .find(|m| m.name().upper_camel_case() == name)
            .unwrap_or_else(|| panic!("`{name}` missing from the dump"));

        assert_eq!(
            format!("{:?}", found.fields()),
            format!("{:?}", model.fields()),
            "`{name}` differs after a round trip"
        );
    }
}

/// A relation is only useful once it points somewhere; a dump that lost the
/// resolved target would still deserialize.
#[test]
fn relations_survive_the_round_trip() {
    let schema = round_trip().schema;

    let order = schema
        .models()
        .find(|m| m.name().upper_camel_case() == "SchemaDumpOrder")
        .unwrap();
    let belongs_to = order
        .fields()
        .iter()
        .find_map(|f| f.ty.as_belongs_to())
        .expect("SchemaDumpOrder belongs to SchemaDumpUser");

    assert_eq!(
        schema.model(belongs_to.target).name().upper_camel_case(),
        "SchemaDumpUser"
    );
}

/// Embedded enums carry a discriminant, which serializes through a codec
/// narrower than `stmt::Value` — one arm per discriminant kind.
#[test]
fn enum_discriminants_survive_the_round_trip() {
    let schema = round_trip().schema;

    let discriminants = |name: &str| -> Vec<Value> {
        schema
            .models()
            .find(|m| m.name().upper_camel_case() == name)
            .expect("embedded enums are discovered through the models that hold them")
            .as_embedded_enum_unwrap()
            .variants
            .iter()
            .map(|v| v.discriminant.clone())
            .collect()
    };

    assert_eq!(
        discriminants("SchemaDumpStatus"),
        vec![
            Value::String("draft".into()),
            Value::String("placed".into())
        ]
    );
    assert_eq!(
        discriminants("SchemaDumpPriority"),
        vec![Value::I64(1), Value::I64(2)]
    );
}

/// The CLI reports a version mismatch rather than a parse failure, which only
/// works if the version rides along with the schema.
#[test]
fn dump_reports_the_toasty_version_that_produced_it() {
    assert!(!round_trip().toasty_version.is_empty());
}
