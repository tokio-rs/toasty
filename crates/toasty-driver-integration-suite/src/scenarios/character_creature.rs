use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Character {
        #[key]
        #[auto]
        id: uuid::Uuid,

        creature: Creature,
    }

    // Both variants carry a `name`, mapped to the same `#[column("name")]`.
    // The two variant fields coalesce into a single shared, nullable
    // `creature_name` column; each variant keeps its own distinct column for
    // its variant-specific attribute (`creature_profession` / `creature_species`).
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[column("name")]
            name: String,
            profession: String,
        },
        #[column(variant = 2)]
        Animal {
            #[column("name")]
            name: String,
            species: String,
        },
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(Character)).await
    }
}
