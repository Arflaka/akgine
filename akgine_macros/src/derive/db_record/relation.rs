/*! Code generation for `#[column(relation)]` fields (foreign keys). */

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type;

use super::attrs::FieldAttrs;

/**
 *  Info kept about each relation field, used later by `preload.rs`.
 */
pub(super) struct RelationInfo {
    /** The SQL column holding the foreign key (e.g. "game_id")*/
    pub col_name_lit: syn::LitStr,
    /** The related type (e.g. `Game`)*/
    pub related_type: Type,
    /** Whether the Rust field is `Option<Related>` (nullable FK)*/
    pub is_option: bool,
}

/// Always an Integer FK referencing the related type's own `table_name()`
/// (read at runtime, so it can never drift out of sync with a rename).
pub(super) fn generate_column_expr(
    fk_col_name_lit: &syn::LitStr,
    related_type: &Type,
    is_option: bool,
    attrs: &FieldAttrs,
) -> TokenStream2 {
    let mut expr: TokenStream2 = quote! {
        ::akgine::database::Column::new(#fk_col_name_lit, ::akgine::database::ColType::Integer)
            .references(<#related_type as ::akgine::database::DbRecord>::table_name(), "id")
    };

    let nullable: bool = is_option && !attrs.not_null;
    if (!nullable) {
        expr = quote! { #expr.not_null() };
    }
    if let Some(default) = &attrs.default {
        expr = quote! { #expr.default(#default) };
    }
    expr
}

/** `getValues` for a relation field: fetch the related row through `_db`.
 *  If the generated `preload()` already ran `find_many` for these ids,
 *  `find(..)` hits the row cache and costs nothing extra.
 */
pub(super) fn generate_get_value_expr(
    ident: &syn::Ident,
    related_type: &Type,
    fk_col_name_lit: &syn::LitStr,
    is_option: bool,
) -> TokenStream2 {
    if is_option {
        quote! {
            #ident: match v.getValue(#fk_col_name_lit)?.as_opt_i64()? {
                Some(fk_id) => Some(
                    _db.getRepository::<#related_type>()
                        .find(fk_id)?
                        .ok_or(::akgine::database::DbError::NotFound)?
                ),
                None => None,
            }
        }
    } else {
        quote! {
            #ident: {
                let fk_id: i64 = v.getValue(#fk_col_name_lit)?.as_i64()?;
                _db.getRepository::<#related_type>()
                    .find(fk_id)?
                    .ok_or(::akgine::database::DbError::NotFound)?
            }
        }
    }
}

/** `toParams` for a relation field: write the related row's own id
 *  (the related row must already be persisted).
 */
pub(super) fn generate_to_params_expr(
    ident: &syn::Ident,
    related_type: &Type,
    fk_col_name_lit: &syn::LitStr,
    fieldName: &str,
    is_option: bool,
) -> TokenStream2 {
    /* path to the trait method: `<Game as akgine::database::DbRecord>::id` */
    let id_fn: TokenStream2 = quote! { <#related_type as ::akgine::database::DbRecord>::id };

    if is_option {
        /* Optional relation: a missing relation is stored as NULL. */
        quote! {
            (#fk_col_name_lit, self.#ident.as_ref().and_then(|r| r.id()).into())
        }
    } else {
        /* The related record MUST have an id, otherwise we panic with `msg`. */
        let msg: String =
            format!("`{fieldName}` must be persisted (have a real id) before saving this record");
        quote! {
            (#fk_col_name_lit, #id_fn(&self.#ident).expect(#msg).into())
        }
    }
}
