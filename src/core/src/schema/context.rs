use super::*;

use std::collections::HashMap;

pub(crate) struct Context {
    /// Maps model names to identifiers
    model_lookup: HashMap<String, ModelId>,

    /// Maps table names to identifiers
    table_lookup: HashMap<String, TableId>,

    /// Relation metadata. This is stored here as it needs to be used at a later stage
    foreign_keys: HashMap<FieldId, model::attr::Relation>,
}

impl Context {
    pub(crate) fn new() -> Context {
        Context {
            model_lookup: HashMap::new(),
            table_lookup: HashMap::new(),
            foreign_keys: HashMap::new(),
        }
    }

    /// Register a model name with an identifier. This will be used later for
    /// mapping.
    pub(crate) fn register_model(&mut self, name: impl AsRef<str>) -> ModelId {
        assert!(!self.model_lookup.contains_key(name.as_ref()));
        let id = ModelId(self.model_lookup.len());
        self.model_lookup.insert(name.as_ref().to_string(), id);
        id
    }

    pub(crate) fn resolve_ty(&self, path: &ast::Path, parent: ModelId) -> stmt::Type {
        assert!(!path.segments.is_empty());

        let segment = &path.segments[0];

        // Check some hard-coded primitive types
        let ret = match segment.ident.as_str() {
            "bool" => stmt::Type::Bool,
            "String" => stmt::Type::String,
            "i64" => stmt::Type::I64,
            "Id" => {
                if let Some(arguments) = &segment.arguments {
                    assert_eq!(1, arguments.arguments.len());

                    match &arguments.arguments[0] {
                        ast::Type::Path(type_path) => {
                            match self.resolve_ty(&type_path.path, parent) {
                                stmt::Type::Model(model_id) => stmt::Type::Id(model_id),
                                _ => todo!(),
                            }
                        }
                        ty => todo!("ty={:#?}", ty),
                    }
                } else {
                    stmt::Type::Id(parent)
                }
            }
            ident => stmt::Type::Model(
                self.model_lookup
                    .get(ident)
                    .copied()
                    .unwrap_or_else(|| panic!("no model named `{ident}`")),
            ),
        };

        if !ret.is_id() {
            assert!(segment.arguments.is_none());

            if !ret.is_model() {
                assert_eq!(1, path.segments.len());
            }
        }

        ret
    }

    pub(crate) fn resolve_model(&self, path: &ast::Path) -> ModelId {
        match self.resolve_ty(path, ModelId::placeholder()) {
            stmt::Type::Model(model_id) => model_id,
            _ => todo!(),
        }
    }

    pub(crate) fn register_table(&mut self, name: impl AsRef<str>) -> table::TableId {
        assert!(!self.table_lookup.contains_key(name.as_ref()));
        let id = table::TableId(self.table_lookup.len());
        self.table_lookup.insert(name.as_ref().to_string(), id);
        id
    }

    pub(crate) fn store_relation_attr(&mut self, field_id: FieldId, attr: model::attr::Relation) {
        assert!(
            !self.foreign_keys.contains_key(&field_id),
            "duplicate relation attribute"
        );
        self.foreign_keys.insert(field_id, attr);
    }

    pub(crate) fn get_relation_attr(&self, field_id: FieldId) -> &model::attr::Relation {
        self.foreign_keys
            .get(&field_id)
            .expect("missing relation attribute")
    }
}
