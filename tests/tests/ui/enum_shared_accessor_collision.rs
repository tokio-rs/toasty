// Shared ident `human` collides with the variant accessor `human()` on the
// same enum. The gateless accessor is named after the ident, so the collision
// must be rejected with a clear error.
#[derive(Debug, toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[shared(human)]
        name: String,
    },
    #[column(variant = 2)]
    Animal {
        #[shared(human)]
        name: String,
    },
}

fn main() {}
