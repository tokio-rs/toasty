// Fields sharing one logical field declare conflicting `#[column("...")]`
// overrides. A shared field maps to exactly one column, so the overrides must
// agree.

#[derive(Debug, toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[shared(name)]
        #[column("a")]
        name: String,
    },
    #[column(variant = 2)]
    Animal {
        #[shared(name)]
        #[column("b")]
        name: String,
    },
}

fn main() {}
