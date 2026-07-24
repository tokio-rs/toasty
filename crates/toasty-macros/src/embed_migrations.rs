use std::{
    collections::HashSet,
    env, fs,
    path::{Component, Path, PathBuf},
};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitStr;
use toasty_core::migration::History;

pub(crate) fn generate(input: TokenStream) -> syn::Result<TokenStream> {
    let path = if input.is_empty() {
        LitStr::new("toasty", Span::call_site())
    } else {
        syn::parse2::<LitStr>(input)?
    };
    let root = resolve_root(&path)?;
    let history_path = root.join("history.toml");
    let history_content = read_utf8(&history_path, &path, "migration history")?;
    let history: History = history_content.parse().map_err(|error| {
        syn::Error::new(
            path.span(),
            format!("failed to parse {}: {error}", history_path.display()),
        )
    })?;

    let migrations_dir = root.join("migrations");
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut migrations = Vec::with_capacity(history.entries().len());

    for entry in history.entries() {
        validate_name(&entry.name, &path)?;
        if !ids.insert(entry.id) {
            return Err(syn::Error::new(
                path.span(),
                format!("duplicate migration id {}", entry.id),
            ));
        }
        if !names.insert(entry.name.as_str()) {
            return Err(syn::Error::new(
                path.span(),
                format!("duplicate migration name {}", entry.name),
            ));
        }

        let sql_path = migrations_dir.join(&entry.name);
        ensure_exists(&sql_path, &path, "migration SQL")?;
        let sql_path = path_literal(&sql_path, path.span())?;
        let name = LitStr::new(&entry.name, path.span());
        let id = entry.id;
        migrations.push(quote! {
            toasty::migration::MigrationFile::new(#id, #name, include_str!(#sql_path))
        });
    }

    let history_path = path_literal(&history_path, path.span())?;
    Ok(quote! {{
        const _: &str = include_str!(#history_path);
        const MIGRATIONS: &[toasty::migration::MigrationFile] = &[#(#migrations),*];
        toasty::migration::MigrationSet::new(MIGRATIONS)
    }})
}

fn resolve_root(path: &LitStr) -> syn::Result<PathBuf> {
    let root = PathBuf::from(path.value());
    let root = if root.is_absolute() {
        root
    } else {
        let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| syn::Error::new(path.span(), "CARGO_MANIFEST_DIR is not available"))?;
        PathBuf::from(manifest_dir).join(root)
    };

    root.canonicalize().map_err(|error| {
        syn::Error::new(
            path.span(),
            format!(
                "failed to open migration directory {}: {error}",
                root.display()
            ),
        )
    })
}

fn validate_name(name: &str, path: &LitStr) -> syn::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(syn::Error::new(
            path.span(),
            format!("migration name must be a file name without path components: {name}"),
        ));
    }
    Ok(())
}

fn read_utf8(path: &Path, input: &LitStr, description: &str) -> syn::Result<String> {
    fs::read_to_string(path).map_err(|error| {
        syn::Error::new(
            input.span(),
            format!("failed to read {description} {}: {error}", path.display()),
        )
    })
}

fn ensure_exists(path: &Path, input: &LitStr, description: &str) -> syn::Result<()> {
    let exists = fs::exists(path).map_err(|error| {
        syn::Error::new(
            input.span(),
            format!("failed to check {description} {}: {error}", path.display()),
        )
    })?;

    if exists {
        Ok(())
    } else {
        Err(syn::Error::new(
            input.span(),
            format!("missing {description} {}", path.display()),
        ))
    }
}

fn path_literal(path: &Path, span: Span) -> syn::Result<LitStr> {
    let path = path.to_str().ok_or_else(|| {
        syn::Error::new(
            span,
            format!("migration path is not valid UTF-8: {}", path.display()),
        )
    })?;
    Ok(LitStr::new(path, span))
}
