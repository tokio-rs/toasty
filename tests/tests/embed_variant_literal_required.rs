//! The `create!` / `update!` variant-literal rewrite maps each written field
//! to a builder setter, so an omitted field produces no call instead of the
//! compile error a plain struct literal would give. The generated builder
//! restores the exhaustiveness check at expression-build time: a required
//! slot left unset — neither written in the literal nor filled from a loaded
//! relation value — panics rather than persisting a `Null` that later fails
//! to decode.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Human {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
    Anonymous {
        label: String,
        note: Option<String>,
    },
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,
    owner: Owner,
}

/// An unloaded relation fills no key slot, so omitting the key field leaves
/// it unset.
#[test]
#[should_panic(
    expected = "cannot build `Owner::Human` expression: key field `id` is not set \
                           and relation `human` has no loaded value to fill it from"
)]
fn unloaded_relation_with_unset_key_panics() {
    let _ = toasty::create!(Object {
        owner: Owner::Human {
            human: toasty::Deferred::default()
        }
    });
}

/// A required non-key field has no relation to fill it from; omitting it is
/// always an error.
#[test]
#[should_panic(
    expected = "cannot build `Owner::Anonymous` expression: missing required field `label`"
)]
fn missing_required_field_panics() {
    let _ = toasty::create!(Object {
        owner: Owner::Anonymous {}
    });
}

/// Nullable fields are not required; leaving one unset stores `None`.
#[test]
fn optional_field_may_stay_unset() {
    let _ = toasty::create!(Object {
        owner: Owner::Anonymous {
            label: "n/a".to_string()
        }
    });
}
