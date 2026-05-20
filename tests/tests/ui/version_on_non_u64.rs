// `#[version]` requires a `u64` field. The macro does not inspect the type
// token to enforce this — it emits a `VersionCounter` obligation that trait
// resolution checks against the resolved type, so a non-`u64` field is rejected
// here (and, conversely, a `u64` type alias would be accepted).

#[derive(toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[version]
    version: i64,
}

fn main() {}
