//! Test that relational fields don't need to be `pub` when models are in separate modules.
//!
//! Regression test for https://github.com/tokio-rs/toasty/issues/335

mod model_a {
    #[derive(Debug, toasty::Model)]
    pub struct A {
        #[key]
        id: uuid::Uuid,

        #[has_one]
        b: toasty::HasOne<super::model_b::B>,
    }
}

mod model_b {
    #[derive(Debug, toasty::Model)]
    pub struct B {
        #[key]
        id: uuid::Uuid,

        #[belongs_to(key = a_id, references = id)]
        a: toasty::BelongsTo<super::model_a::A>,
        a_id: uuid::Uuid,
    }
}

/// If this compiles, the issue is fixed: private relation fields across modules work.
#[test]
fn relation_fields_can_be_private_across_modules() {
    // Verify that the generated API methods are accessible from outside the modules.
    let _ = model_a::A::all();
    let _ = model_b::B::all();
}
