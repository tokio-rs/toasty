// Enum-level `#[unique(...)]` references: an unknown identifier, and a
// non-shared variant field by bare name (which requires the qualified
// `variant::field` form).

#[derive(Debug, toasty::Embed)]
#[unique(nope)]
#[index(profession)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[shared(name)]
        name: String,
        profession: String,
    },
    #[column(variant = 2)]
    Animal {
        #[shared(name)]
        name: String,
    },
}

fn main() {}
