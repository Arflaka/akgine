//! Resolves the SQL table name.

use syn::DeriveInput;

/// 1) `#[table("...")]`
/// 2) struct name + "s"
pub(super) fn resolve_table_name(ast: &DeriveInput) -> syn::Result<String> {
    for attr in &ast.attrs {
        if (attr.path().is_ident("table")) {
            let tableName: syn::LitStr = attr.parse_args()?;
            return Ok(tableName.value());
        }
    }
    Ok(ast.ident.to_string() + "s")
}
