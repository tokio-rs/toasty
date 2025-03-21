use toasty_schema::*;

use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

macro_rules! assert_parse {
    (
        $fixture:expr,
        $schema:expr
    ) => {{
        let res = toasty_schema::from_file(crate::parse::fixture($fixture)).unwrap();
        pretty_assertions::assert_eq!(res, $schema);
    }};
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(ROOT).join(format!("tests/fixtures/{}.toasty", name))
}

pub fn index(model: usize, index: usize) -> IndexId {
    IndexId {
        model: ModelId(model),
        index,
    }
}

pub fn field(model: usize, index: usize) -> FieldId {
    FieldId {
        model: ModelId(model),
        index,
    }
}

pub fn hash_index_field(field: FieldId) -> IndexField {
    IndexField {
        field,
        kind: IndexKind::Hash,
    }
}
