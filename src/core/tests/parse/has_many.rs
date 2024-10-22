use super::util::field;

use toasty_schema::*;

#[test]
fn user_todo() {
    assert_parse!(
        "user_todo",
        Schema {
            models: vec![
                Model {
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
                            name: "todos".to_string(),
                            column_name: None,
                            ty: Type::HasMany(relation::HasMany {
                                target: ModelId(1),
                                pair: FieldId {
                                    model: ModelId(1),
                                    index: 2,
                                },
                                foreign_key_name: "user_id".to_string(),
                            }),
                            nullable: false,
                            auto: None,
                        }
                    ],
                    primary_key: 0,
                    indices: vec![],
                },
                Model {
                    id: ModelId(1),
                    name: "Todo".to_string(),
                    table_name: "todos".to_string(),
                    fields: vec![
                        Field {
                            id: field(1, 0),
                            name: "id".to_string(),
                            column_name: Some("id".to_string()),
                            ty: Type::Id,
                            nullable: false,
                            auto: Some(Auto::Id),
                        },
                        Field {
                            id: field(1, 1),
                            name: "title".to_string(),
                            column_name: Some("title".to_string()),
                            ty: Type::String,
                            nullable: false,
                            auto: None,
                        },
                        Field {
                            id: field(1, 2),
                            name: "user".to_string(),
                            column_name: Some("user".to_string()),
                            ty: Type::BelongsTo(relation::BelongsTo {
                                target: ModelId(0),
                                foreign_key_name: "user_id".to_string(),
                            }),
                            nullable: false,
                            auto: None,
                        }
                    ],
                    primary_key: 0,
                    indices: vec![],
                }
            ],
        }
    );
}
