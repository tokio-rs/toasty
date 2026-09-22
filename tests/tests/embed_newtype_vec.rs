#[derive(toasty::Embed)]
struct List(Vec<String>);

#[test]
fn derive_embed_for_vec_newtype() {
    let _ = List(vec!["one".to_owned()]);
}
