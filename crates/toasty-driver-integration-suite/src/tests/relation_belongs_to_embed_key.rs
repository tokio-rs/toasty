use crate::prelude::*;

/// Regression test for tokio-rs/toasty#897: a `BelongsTo` whose foreign key
/// references a primary key that is an `Embed` newtype caused the derive macro
/// to emit code that called `Field::key_constraint` with a `{Embed}Fields`
/// wrapper instead of the underlying `Path`, leading to a compile error.
#[driver_test(requires(sql))]
pub async fn belongs_to_with_embed_pk(t: &mut Test) -> Result<()> {
    #[derive(Clone, Debug, PartialEq, Eq, toasty::Embed)]
    pub struct MeetingId(String);

    #[derive(Clone, Debug, PartialEq, Eq, toasty::Embed)]
    pub struct AgendaItemId(String);

    #[derive(Debug, toasty::Model)]
    pub struct Meeting {
        #[key]
        pub id: MeetingId,

        #[has_many]
        pub agenda_items: toasty::HasMany<AgendaItem>,
    }

    #[derive(Debug, toasty::Model)]
    pub struct AgendaItem {
        #[key]
        pub id: AgendaItemId,

        #[index]
        pub meeting_id: MeetingId,

        #[belongs_to(key = meeting_id, references = id)]
        pub meeting: toasty::BelongsTo<Meeting>,
    }

    let mut db = t
        .setup_db(models!(Meeting, AgendaItem, MeetingId, AgendaItemId))
        .await;

    let meeting = toasty::create!(Meeting {
        id: MeetingId("m1".into()),
    })
    .exec(&mut db)
    .await?;

    let item = toasty::create!(AgendaItem {
        id: AgendaItemId("a1".into()),
        meeting: &meeting,
    })
    .exec(&mut db)
    .await?;

    assert_eq!(item.meeting_id.0, "m1");

    // The accessor whose codegen used to fail to compile.
    let loaded = item.meeting().exec(&mut db).await?;
    assert_eq!(loaded.id.0, "m1");

    Ok(())
}
