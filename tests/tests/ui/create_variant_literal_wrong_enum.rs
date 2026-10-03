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

#[derive(Debug, toasty::Embed)]
enum Other {
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
    owner: Owner,
}

fn main() {
    let alice = Human {
        id: uuid::Uuid::nil(),
    };
    // The literal builds an `Other` expression; the `owner` setter takes
    // an `Owner` and must reject it.
    let _ = toasty::create!(Object {
        owner: Other::Human { human: &alice }
    });
}
