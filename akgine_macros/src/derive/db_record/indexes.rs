/*! Parsing and validation of struct-level `#[index(...)]` attributes. */

use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, DeriveInput, LitStr, Token, punctuated::Punctuated};

/*
 *  Verifies that:
 *  1. every column named in an index exists (per `valid_columns`);
 *  2. no index is duplicated (`("A","B")` == `("B","A")`).
 */
// pub(super) fn resolve_indexes(
//     ast: &DeriveInput,
//     valid_columns: &HashSet<String>,
// ) -> syn::Result<Vec<TokenStream2>> {
//     /* store index expression */
//     let mut indexes_exprs: Vec<TokenStream2> = Vec::new();
//     /* sorted column lists, to detect duplicates regardless of order */
//     let mut seen_indexes: HashSet<Vec<String>> = HashSet::new();

//     for attr in &ast.attrs {
//         /* we care only about `#[index()]` */
//         if !attr.path().is_ident("index") {
//             continue;
//         }

//         /* parse `#[index("col1", "col2", ...)]`
//         make a list of all "colX" separete by comma */
//         let nested: syn::punctuated::Punctuated<syn::LitStr, syn::token::Comma> = attr
//             .parse_args_with(
//                 syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
//             )?;

//         /* store all columns name */
//         let mut cols: Vec<String> = Vec::new();
//         /* travel all column give in params */
//         for lit in nested {
//             /* get the name of the column */
//             let col_name: String = lit.value();
//             /* check if the column exist */
//             if (!valid_columns.contains(&col_name)) {
//                 return Err(syn::Error::new(
//                     lit.span(),
//                     format!(
//                         "Column '{}' does not exist in the struct or is skipped",
//                         col_name
//                     ),
//                 ));
//             }
//             cols.push(col_name);
//         }

//         if cols.is_empty() {
//             return Err(syn::Error::new_spanned(
//                 attr,
//                 "Index must contain at least one column",
//             ));
//         }

//         /* sort the columns inside the index */
//         let mut sorted_cols: Vec<String> = cols.clone();
//         sorted_cols.sort();
//         /* try to insert the index and check if it already exist */
//         if (!seen_indexes.insert(sorted_cols)) {
//             return Err(syn::Error::new_spanned(
//                 attr,
//                 "Duplicate index combination detected (order does not matter)",
//             ));
//         }

//         /* convert all string in lits */
//         let col_lits = cols
//             .iter()
//             .map(|c| syn::LitStr::new(c, proc_macro2::Span::call_site()));

//         /* add the expression to the list */
//         indexes_exprs.push(quote! {
//             ::akgine::database::IndexDef::new(&[ #(#col_lits),* ])
//         });
//     }

//     Ok(indexes_exprs)
// }

/**
 *  - `#[index("a", "b")]`  -> plain index (faster lookups, duplicates allowed);
 *  - `#[unique("a", "b")]` -> unique index (the database rejects duplicates).
 *
 *  Verifies that:
 *  1. every column named in an index exists (per `valid_columns`);
 *  2. no index is duplicated (`("A","B")` == `("B","A")`, `index` or `unique`).
 */
pub(super) fn resolve_indexes(
    ast: &DeriveInput,
    valid_columns: &HashSet<String>,
) -> syn::Result<Vec<TokenStream2>> {
    /* sorted column lists, to detect duplicates regardless of order */
    let mut seen_indexes: HashSet<Vec<String>> = HashSet::new();

    ast.attrs
        .iter()
        /* keep only index attributes and remember if each one is unique */
        .filter_map(|attr| index_kind(attr).map(|unique| (attr, unique)))
        .map(|(attr, unique)| resolve_index(attr, unique, valid_columns, &mut seen_indexes))
        /* stops at the first error, like `?` would */
        .collect()
}

/**
 *  `Some(false)` for `#[index]`, `Some(true)` for `#[unique]`, `None` for any other attribute.
 */
fn index_kind(attr: &Attribute) -> Option<bool> {
    if (attr.path().is_ident("index")) {
        Some(false)
    } else if (attr.path().is_ident("unique")) {
        Some(true)
    } else {
        None
    }
}

/**
 *  Parses and validates one attribute, then generates its `IndexDef` expression.
 */
fn resolve_index(
    attr: &Attribute,
    unique: bool,
    valid_columns: &HashSet<String>,
    seen_indexes: &mut HashSet<Vec<String>>,
) -> syn::Result<TokenStream2> {
    /* parse `#[index("col1", "col2", ...)]` into a list of string literals */
    let cols: Punctuated<LitStr, Token![,]> =
        attr.parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)?;

    if (cols.is_empty()) {
        return Err(syn::Error::new_spanned(
            attr,
            "Index must contain at least one column",
        ));
    }

    /* found column that not exist */
    if let Some(unknown) = cols
        /* travel all columns */
        .iter()
        /* find column that don't exist */
        .find(|lit| !valid_columns.contains(&lit.value()))
    {
        return Err(syn::Error::new(
            unknown.span(),
            format!(
                "Column '{}' does not exist in the struct or is skipped",
                unknown.value()
            ),
        ));
    }

    /* sort the columns so that the order does not matter for the duplicate check */
    let mut sorted_cols: Vec<String> = cols.iter().map(LitStr::value).collect();
    sorted_cols.sort();
    /* `insert` returns false when the combination was already declared */
    if (!seen_indexes.insert(sorted_cols)) {
        return Err(syn::Error::new_spanned(
            attr,
            "Duplicate index combination detected (order does not matter)",
        ));
    }

    let col_lits: Vec<&LitStr> = cols.iter().collect();
    /* `.unique()` is only appended for `#[unique(..)]` */
    let unique_call: TokenStream2 = if (unique) {
        quote! { .unique() }
    } else {
        TokenStream2::new()
    };

    Ok(quote! {
        ::akgine::database::IndexDef::new(&[ #(#col_lits),* ])#unique_call
    })
}
