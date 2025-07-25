use tests::*;

use toasty::stmt::Id;

macro_rules! def_num_ty_tests {
    (
        $( $t:ty => $required:ident; )*
    ) => {
        $(
            // The `val` field is used in the test assertions below, but the compiler
            // incorrectly flags it as dead code due to macro expansion.
            // See: https://github.com/rust-lang/rust/issues/102217
            #[allow(dead_code)]
            async fn $required(s: impl Setup) {
                #[derive(Debug, toasty::Model)]
                struct Foo {
                    #[key]
                    #[auto]
                    id: Id<Self>,

                    val: $t,
                }

                let db = s.setup(models!(Foo)).await;

                let mut created = Foo::create()
                    .val(0)
                    .exec(&db)
                    .await
                    .unwrap();

                let read = Foo::get_by_id(&db, &created.id)
                    .await
                    .unwrap();

                assert_eq!(read.val, 0);

                created.update()
                    .val(1)
                    .exec(&db)
                    .await
                    .unwrap();

                let read = Foo::get_by_id(&db, &created.id)
                    .await
                    .unwrap();

                assert_eq!(read.val, 1);
            }
        )*

        tests!(
            $( $required, )*
        );
    };
}

def_num_ty_tests!(
    i64 => required_i64;
    i32 => required_i32;
);
