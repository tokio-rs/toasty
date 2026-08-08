// Field-level `#[unique]` / `#[index]` on a field declaring `#[shared]` would
// constrain rows of every variant sharing the column while reading as
// variant-scoped. The enum-level form is the explicit equivalent.

#[derive(Debug, toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[unique]
        #[shared(name)]
        name: String,
    },
    #[column(variant = 2)]
    Animal {
        #[index]
        #[shared(name)]
        name: String,
    },
}

fn main() {}
