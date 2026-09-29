//! `#[derive(DbRecord)]` implementation.
//!
//! Where to look when something breaks:
//! - attribute parsing (`#[column(..)]`)  -> `attrs.rs`
//! - table name                           -> `table.rs`
//! - Rust type -> SQL type / `as_xxx()`   -> `types.rs`
//! - plain column code generation         -> `columns.rs`
//! - foreign keys (`relation`)            -> `relation.rs`
//! - batched `preload()`                  -> `preload.rs`
//! - `#[index(..)]` validation            -> `indexes.rs`
//! - per-field dispatch loop              -> `fields.rs`
//! - final `impl DbRecord` assembly       -> this file

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

/// Entry point called from `lib.rs` through `derive::run`.
pub fn expand(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_name: &syn::Ident = &ast.ident;
    let tableName: String = table::resolve_table_name(ast)?;
    let named_fields = named_fields(ast)?;

    /* generate the per-field code fragments */
    let frags: fields::Fragments = fields::process(named_fields)?;

    /* indexes are validated once all columns are known */
    let indexes_exprs = indexes::resolve_indexes(ast, &frags.valid_columns)?;

    /* empty for relation-free structs (trait default no-op applies) */
    let preload_impl: TokenStream2 = preload::generate_preload_impl(&frags.relations);

    let col_exprs = &frags.col_exprs;
    let get_values_exprs = &frags.get_values_exprs;
    let to_params_exprs = &frags.to_params_exprs;

    Ok(quote! {
        impl ::akgine::database::DbRecord for #struct_name {
            fn table_name() -> &'static str {
                #tableName
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

/// Only structs with named fields are supported.
fn named_fields(ast: &DeriveInput) -> syn::Result<&Punctuated<Field, Comma>> {
    match &ast.data {
        Data::Struct(s) => match &s.fields {
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
