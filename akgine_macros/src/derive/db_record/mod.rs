/*! Where to look when something breaks:
 *  - attribute parsing (`#[column(..)]`)  -> `attrs.rs`
 *  - table name                           -> `table.rs`
 *  - Rust type -> SQL type / `as_xxx()`   -> `types.rs`
 *  - plain column code generation         -> `columns.rs`
 *  - foreign keys (`relation`)            -> `relation.rs`
 *  - batched `preload()`                  -> `preload.rs`
 *  - `#[index(..)]` validation            -> `indexes.rs`
 *  - per-field dispatch loop              -> `fields.rs`
 *  - final `impl DbRecord` assembly       -> this file
 */

mod attrs;
mod columns;
mod fields;
mod indexes;
mod preload;
mod relation;
mod table;
mod types;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, punctuated::Punctuated, token::Comma};

/**
 * Entry point
 */
pub fn expand(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    /* name of the struct */
    let struct_name: &syn::Ident = &ast.ident;
    /* name of the table */
    let table_name: String = table::resolve_table_name(ast)?;
    /* a punctuated with all field (also check if all fields is named) */
    let named_fields: &Punctuated<Field, Comma> = named_fields(ast)?;

    /* generate the per-field code fragments */
    let frags: fields::Fragments = fields::process(named_fields)?;

    /* indexes are validated once all columns are known */
    let indexes_exprs: Vec<TokenStream2> = indexes::resolve_indexes(ast, &frags.valid_columns)?;

    /* generate a preload function or empty for relation-free structs */
    let preload_impl: TokenStream2 = preload::generate_preload_impl(&frags.relations);

    /* get all expretions */
    let col_exprs: &Vec<TokenStream2> = &frags.col_exprs;
    let get_values_exprs: &Vec<TokenStream2> = &frags.get_values_exprs;
    let to_params_exprs: &Vec<TokenStream2> = &frags.to_params_exprs;

    Ok(quote! {
        impl ::akgine::database::DbRecord for #struct_name {
            fn table_name() -> &'static str {
                #table_name
            }

            fn columns() -> Vec<::akgine::database::Column> {
                vec![
                    #(#col_exprs),*
                ]
            }

            fn indexes() -> Vec<::akgine::database::IndexDef> {
                vec![
                    #(#indexes_exprs),*
                ]
            }

            fn getValues(v: &::akgine::database::ValueSet, _db: &::akgine::database::DataBase) -> Result<Self, ::akgine::database::DbError> {
                Ok(Self {
                    #(#get_values_exprs),*
                })
            }

            fn toParams(&self) -> Vec<(&'static str, ::akgine::database::SqlValue)> {
                vec![
                    #(#to_params_exprs),*
                ]
            }

            fn id(&self) -> Option<i64> {
                if self.id > 0 { Some(self.id) } else { None }
            }

            fn set_id(&mut self, id: i64) {
                self.id = id;
            }

            #preload_impl
        }
    })
}

/**
 * get a `&Punctuated<Field, Comma>` on a ast if the ast is a struct with named fields
 */
fn named_fields(ast: &DeriveInput) -> syn::Result<&Punctuated<Field, Comma>> {
    /* check if the derive is on a structs and not on a enum or union */
    match &ast.data {
        /* check if the fields is named */
        Data::Struct(s) => match &s.fields {
            /* if it's named return the punctuated */
            Fields::Named(f) => Ok(&f.named),
            _ => Err(syn::Error::new_spanned(
                &ast.ident,
                "DbRecord requires a struct with named fields",
            )),
        },
        _ => Err(syn::Error::new_spanned(
            &ast.ident,
            "DbRecord can only be derived for structs",
        )),
    }
}
