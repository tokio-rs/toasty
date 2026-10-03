//! A variant literal value that copies out of the update target is
//! evaluated before `.update()` borrows the target, so it compiles. The
//! borrowing counterpart is `tests/ui/update_variant_literal_borrows_target.rs`.
#![allow(dead_code)]

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Human {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,
    #[index]
    mirror_id: uuid::Uuid,
    #[belongs_to(key = mirror_id)]
    mirror: toasty::Deferred<Human>,
    owner: Owner,
}

fn main() {
    let mut object = Object {
        id: uuid::Uuid::nil(),
        mirror_id: uuid::Uuid::nil(),
        mirror: toasty::Deferred::default(),
        owner: Owner::Human {
            id: uuid::Uuid::nil(),
            human: toasty::Deferred::default(),
        },
    };
    let _ = toasty::update!(object {
        owner: Owner::Human {
            id: object.mirror_id,
            human: toasty::Deferred::default(),
        }
    });
}
