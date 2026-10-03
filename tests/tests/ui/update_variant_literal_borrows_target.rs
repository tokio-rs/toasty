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
    // The value borrows the target, and the borrow is held across
    // `object.update()`. Copy the key out of the target instead:
    // `Owner::Human { id: object.mirror_id, human: Deferred::default() }`.
    let _ = toasty::update!(object {
        owner: Owner::Human { human: &object.mirror }
    });
}
