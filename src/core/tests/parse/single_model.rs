use super::util::*;

use toasty_schema::*;

#[test]
fn empty() {
    assert_parse!("empty", Schema { models: vec![] });
}

#[test]
fn basic() {
    assert_parse!(
        "single_model_basic",
        Schema {
            models: vec![Model {
                id: ModelId(0),
                name: "User".to_string(),
                table_name: "users".to_string(),
                fields: vec![
                    Field {
                        id: field(0, 0),
                        name: "id".to_string(),
                        column_name: Some("id".to_string()),
                        ty: Type::Id,
                        nullable: false,
                        auto: Some(Auto::Id),
                    },
                    Field {
                        id: field(0, 1),
                        name: "name".to_string(),
                        column_name: Some("name".to_string()),
                        ty: Type::String,
                        nullable: false,
                        auto: None,
                    },
                ],
                primary_key: 0,
                indices: vec![],
            }],
        }
    );
}

#[test]
fn single_index() {
    assert_parse!(
        "single_model_single_index",
        Schema {
            models: vec![Model {
                id: ModelId(0),
                name: "User".to_string(),
                table_name: "users".to_string(),
                fields: vec![
                    Field {
                        id: field(0, 0),
                        name: "id".to_string(),
                        column_name: Some("id".to_string()),
                        ty: Type::Id,
                        nullable: false,
                        auto: Some(Auto::Id),
                    },
                    Field {
                        id: field(0, 1),
                        name: "name".to_string(),
                        column_name: Some("name".to_string()),
                        ty: Type::String,
                        nullable: false,
                        auto: None,
                    },
                    Field {
                        id: field(0, 2),
                        name: "email".to_string(),
                        column_name: Some("email".to_string()),
                        ty: Type::String,
                        nullable: false,
                        auto: None,
                    }
                ],
                primary_key: 0,
                indices: vec![Index {
                    id: index(0, 0),
                    fields: vec![hash_index_field(field(0, 2))],
                    unique: true,
                }],
            }],
        }
    );
}
