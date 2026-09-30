/*! Code generation for plain (non-relation) columns. */

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type;

use super::attrs::FieldAttrs;
use super::types::{map_rust_type, map_rust_type_to_as_method};

/**
 *  `Column::new(name, type)` and if require `.not_null()` / `.default(..)`.
 */
pub(super) fn generate_column_expr(
    attrs: &FieldAttrs,
    colName: &str,
    active_type: &Type,
    is_option: bool,
) -> syn::Result<TokenStream2> {
    /* get the type of the column in rust */
    let colType: TokenStream2 = map_rust_type(active_type)?;
    /* make the start of the expression */
    let mut expr: TokenStream2 = quote! { ::akgine::database::Column::new(#colName, #colType) };

    /* if the element can be null */
    let nullable: bool = (is_option || attrs.nullable) && !attrs.not_null;
    if (!nullable) {
        expr = quote! { #expr.not_null() };
    }
    /* check and get the default value */
    if let Some(default) = &attrs.default {
        expr = quote! { #expr.default(#default) };
    }
    Ok(expr)
}

/**
 *  `field: v.getValue("col")?.as_xxx()?`
 */
pub(super) fn generate_get_value_expr(
    ident: &syn::Ident,
    active_type: &Type,
    col_name_lit: &syn::LitStr,
) -> syn::Result<TokenStream2> {
    let as_method: syn::Ident = map_rust_type_to_as_method(active_type)?;
    Ok(quote! { #ident: v.getValue(#col_name_lit)?.#as_method()? })
}

/**
 *  `("col", self.field.clone().into())`
 */
pub(super) fn generate_to_params_expr(
    ident: &syn::Ident,
    col_name_lit: &syn::LitStr,
) -> TokenStream2 {
    quote! { (#col_name_lit, self.#ident.clone().into()) }
}
