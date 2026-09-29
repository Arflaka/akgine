/*! Parsing and validation of struct-level `#[index(...)]` attributes. */

use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

/**
 *  Verifies that:
 *  1. every column named in an index exists (per `valid_columns`);
 *  2. no index is duplicated (`("A","B")` == `("B","A")`).
 */
pub(super) fn resolve_indexes(
    ast: &DeriveInput,
    valid_columns: &HashSet<String>,
) -> syn::Result<Vec<TokenStream2>> {
    /* store index expression */
    let mut indexes_exprs: Vec<TokenStream2> = Vec::new();
    /* sorted column lists, to detect duplicates regardless of order */
    let mut seen_indexes: HashSet<Vec<String>> = HashSet::new();

    for attr in &ast.attrs {
        /* we care only about `#[index()]` */
        if !attr.path().is_ident("index") {
            continue;
        }

        /* parse `#[index("col1", "col2", ...)]`
        make a list of all "colX" separete by comma */
        let nested: syn::punctuated::Punctuated<syn::LitStr, syn::token::Comma> = attr
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
            )?;

        /* store all columns name */
        let mut cols: Vec<String> = Vec::new();
        /* travel all column give in params */
        for lit in nested {
            /* get the name of the column */
            let col_name: String = lit.value();
            /* check if the column exist */
            if (!valid_columns.contains(&col_name)) {
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

        /* sort the columns inside the index */
        let mut sorted_cols: Vec<String> = cols.clone();
        sorted_cols.sort();
        /* try to insert the index and check if it already exist */
        if (!seen_indexes.insert(sorted_cols)) {
            return Err(syn::Error::new_spanned(
                attr,
                "Duplicate index combination detected (order does not matter)",
            ));
        }

        /* convert all string in lits */
        let col_lits = cols
            .iter()
            .map(|c| syn::LitStr::new(c, proc_macro2::Span::call_site()));

        /* add the expression to the list */
        indexes_exprs.push(quote! {
            ::akgine::database::IndexDef::new(&[ #(#col_lits),* ])
        });
    }

    Ok(indexes_exprs)
}
