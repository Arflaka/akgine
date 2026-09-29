//! Parsing and validation of struct-level `#[index(...)]` attributes.

use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

/// Verifies that:
/// 1. every column named in an index exists (per `valid_columns`);
/// 2. no index is duplicated (`("A","B")` == `("B","A")`).
pub(super) fn resolve_indexes(
    ast: &DeriveInput,
    valid_columns: &HashSet<String>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut indexes_exprs: Vec<TokenStream2> = Vec::new();
    /* sorted column lists, to detect duplicates regardless of order */
    let mut seen_indexes: HashSet<Vec<String>> = HashSet::new();

    for attr in &ast.attrs {
        if !attr.path().is_ident("index") {
            continue;
        }

        /* parse `#[index("col1", "col2", ...)]` */
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        )?;

        let mut cols: Vec<String> = Vec::new();
        for lit in nested {
            let col_name: String = lit.value();
            if !valid_columns.contains(&col_name) {
                return Err(syn::Error::new(
                    lit.span(),
                    format!(
                        "Column '{}' does not exist in the struct or is skipped",
                        col_name
                    ),
                ));
            }
            cols.push(col_name);
        }

        if cols.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "Index must contain at least one column",
            ));
        }

        let mut sorted_cols: Vec<String> = cols.clone();
        sorted_cols.sort();
        if !seen_indexes.insert(sorted_cols) {
            return Err(syn::Error::new_spanned(
                attr,
                "Duplicate index combination detected (order does not matter)",
            ));
        }

        let col_lits = cols
            .iter()
            .map(|c| syn::LitStr::new(c, proc_macro2::Span::call_site()));

        /* fully qualified so the user does not need to import IndexDef */
        indexes_exprs.push(quote! {
            ::akgine::database::IndexDef::new(&[ #(#col_lits),* ])
        });
    }

    Ok(indexes_exprs)
}
