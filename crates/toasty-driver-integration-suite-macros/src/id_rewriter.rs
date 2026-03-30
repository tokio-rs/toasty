use syn::{Type, TypePath, visit_mut::VisitMut};

/// Visitor that rewrites type references to a configurable target type
pub(crate) struct IdRewriter {
    /// The identifier to replace (e.g., "ID")
    ident: String,
    /// The target type to replace with
    target_type: Type,
}

impl IdRewriter {
    pub(crate) fn new(ident: &str, target_type: Type) -> Self {
        Self {
            ident: ident.to_string(),
            target_type,
        }
    }
}

impl VisitMut for IdRewriter {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(TypePath { qself: None, path }) = ty {
            // Check if this matches the identifier we're looking for
            if path.segments.len() == 1 && path.segments[0].ident == self.ident {
                // Replace with target type
                *ty = self.target_type.clone();
                return;
            }
        }

        // Continue visiting nested types
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
