use crate::ast;

#[derive(Debug)]
pub(super) enum Model {
    Key(Key),
}

#[derive(Debug)]
pub(super) enum Field {
    Auto,
    Index,
    Key,
    Relation(Relation),
    Unique,
}

/// Set of all attributes for a single field
pub(super) struct FieldSet {
    attrs: Vec<Field>,
}

#[derive(Debug, Clone)]
pub(crate) struct Relation {
    pub(crate) key: Vec<ast::Ident>,
    pub(crate) references: Vec<ast::Ident>,
}

#[derive(Debug)]
pub(super) struct Key {
    // Names of fields that make up the partition key
    pub(super) partition: Vec<String>,

    // Name of fields that make up the local key
    pub(super) local: Vec<String>,
}

impl Model {
    pub(super) fn from_ast(ast: &ast::Attribute) -> Model {
        match ast.meta.ident().as_str() {
            "key" => Model::Key(Model::key_from_ast(&ast.meta)),
            _ => todo!("attribute = {:#?}", ast),
        }
    }

    fn key_from_ast(meta: &ast::Meta) -> Key {
        let mut partition = vec![];
        let mut local = vec![];

        for meta in &meta.as_list().items {
            match meta {
                ast::Meta::NameValue(name_value) => match name_value.name.as_str() {
                    "partition" => {
                        assert!(partition.is_empty());
                        partition = expr_to_string_vec(&name_value.value);
                    }
                    "local" => {
                        assert!(local.is_empty());
                        local = expr_to_string_vec(&name_value.value);
                    }
                    _ => todo!(),
                },
                _ => todo!(),
            }
        }

        Key { partition, local }
    }
}

impl FieldSet {
    pub(super) fn from_ast(ast: &[ast::Attribute]) -> FieldSet {
        let attrs: Vec<_> = ast.iter().map(Field::from_ast).collect();

        // Validate attributes
        for (i, attr_i) in attrs.iter().enumerate() {
            if attr_i.is_index() {
                for attr_j in &attrs[(i + 1)..] {
                    if attr_j.is_index() {
                        panic!("field has more than one index");
                    }
                }
            } else if attr_i.is_relation() {
                for attr_j in &attrs[(i + 1)..] {
                    if attr_j.is_relation() {
                        panic!("field has more than one relation attribute");
                    }
                }
            }
        }

        FieldSet { attrs }
    }

    pub(super) fn relation(&self) -> Option<&Relation> {
        self.attrs
            .iter()
            .filter_map(|attr| match attr {
                Field::Relation(relation) => Some(relation),
                _ => None,
            })
            .next()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Field> + '_ {
        self.attrs.iter()
    }
}

impl Field {
    pub(super) fn from_ast(ast: &ast::Attribute) -> Field {
        match ast.meta.ident().as_str() {
            "auto" => Field::Auto,
            "index" => Field::Index,
            "key" => Field::Key,
            "unique" => Field::Unique,
            "relation" => Field::Relation(Relation::from_ast(&ast.meta)),
            _ => todo!("attribute = {:#?}", ast),
        }
    }

    pub(super) fn is_auto(&self) -> bool {
        matches!(self, Field::Auto)
    }

    pub(super) fn is_index(&self) -> bool {
        matches!(self, Field::Index | Field::Unique)
    }

    pub(super) fn is_key(&self) -> bool {
        matches!(self, Field::Key)
    }

    pub(super) fn is_relation(&self) -> bool {
        matches!(self, Field::Relation(..))
    }

    pub(super) fn is_unique(&self) -> bool {
        matches!(self, Field::Unique)
    }
}

impl Relation {
    fn from_ast(ast: &ast::Meta) -> Relation {
        let mut relation = Relation {
            key: vec![],
            references: vec![],
        };

        match ast {
            ast::Meta::Ident(ident) => {
                assert_eq!("relation", ident.as_str());
            }
            ast::Meta::List(list) => {
                for item in &list.items {
                    match item {
                        ast::Meta::NameValue(name_value) => match name_value.name.as_str() {
                            "references" => match &name_value.value {
                                ast::Expr::Ident(ident) => {
                                    relation.references.push(ident.clone());
                                }
                            },
                            "key" => match &name_value.value {
                                ast::Expr::Ident(ident) => {
                                    relation.key.push(ident.clone());
                                }
                            },
                            _ => todo!("meta = {:#?}", ast),
                        },
                        _ => todo!(),
                    }
                }
            }
            ast::Meta::NameValue(_) => todo!("{:#?}", ast),
        }

        relation
    }
}

fn expr_to_string_vec(expr: &ast::Expr) -> Vec<String> {
    match expr {
        ast::Expr::Ident(ident) => vec![ident.to_string()],
    }
}
