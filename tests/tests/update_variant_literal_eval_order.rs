//! `update!` evaluates the values written in a variant literal in field
//! order, together with its other values, and evaluates the target once
//! after all of them.

use std::cell::RefCell;

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
    title: String,
    owner: Owner,
}

#[test]
fn values_left_to_right_then_target() {
    let log = RefCell::new(Vec::new());
    let step = |name: &'static str| {
        log.borrow_mut().push(name);
        name.to_string()
    };
    let target = |id: uuid::Uuid| {
        step("target");
        Object::filter_by_id(id)
    };

    let _ = toasty::update!(target(uuid::Uuid::nil()) {
        title: step("title"),
        owner: Owner::Anonymous {
            label: step("label"),
            note: Some(step("note")),
        },
    });

    assert_eq!(*log.borrow(), ["title", "label", "note", "target"]);
}
