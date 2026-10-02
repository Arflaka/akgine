/*! Walks every struct field and dispatches it to the right generator
 *  (id / skipped / relation / plain column).
 */

use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Field, punctuated::Punctuated, token::Comma};

use super::attrs::FieldAttrs;
use super::columns;
use super::relation::{self, RelationInfo};
use crate::util::option::unwrap_option;

/** Everything collected while walking the fields. */
pub(super) struct Fragments {
    pub col_exprs: Vec<TokenStream2>,
    pub get_values_exprs: Vec<TokenStream2>,
    pub to_params_exprs: Vec<TokenStream2>,
    pub relations: Vec<RelationInfo>,
    /** All valid SQL column names, used to verify `#[index(..)]`. */
    pub valid_columns: HashSet<String>,
}

pub(super) fn process(named_fields: &Punctuated<Field, Comma>) -> syn::Result<Fragments> {
    let mut out = Fragments {
        col_exprs: Vec::new(),
        get_values_exprs: Vec::new(),
        to_params_exprs: Vec::new(),
        relations: Vec::new(),
        valid_columns: HashSet::new(),
    };
    /* id is always handled and available */
    out.valid_columns.insert("id".to_string());

    for field in named_fields {
        process_field(field, &mut out)?;
    }
    Ok(out)
}

fn process_field(field: &Field, out: &mut Fragments) -> syn::Result<()> {
    let ident: &syn::Ident = field.ident.as_ref().unwrap();
    let fieldName: String = ident.to_string();

    /* Rule 1: `id` only appears in getValues */
    if (fieldName == "id") {
        out.get_values_exprs
            .push(quote! { id: v.getValue("id")?.as_i64()? });
        return Ok(());
    }

    let attrs: FieldAttrs = FieldAttrs::parse(field)?;

    /* `#[column(skip)]`: no column, so the value can never come from the row.
    `Default::default()` keeps the field disconnected from the DB. */
    if (attrs.skip) {
        out.get_values_exprs
            .push(quote! { #ident: ::core::default::Default::default() });
        return Ok(());
    }

    let (is_option, inner) = unwrap_option(&field.ty);
    let active_type: &syn::Type = inner.unwrap_or(&field.ty);

    /* relation fields: the SQL column stores the related row's id */
    if (attrs.foreignKey) {
        let fk_col_name: String = attrs
            .name
            .clone()
            .unwrap_or_else(|| format!("{fieldName}_id"));
        let fk_lit: syn::LitStr = syn::LitStr::new(&fk_col_name, ident.span());
        let related_type: syn::Type = active_type.clone();

        out.col_exprs.push(relation::generate_column_expr(
            &fk_lit,
            &related_type,
            is_option,
            &attrs,
        ));
        out.get_values_exprs.push(relation::generate_get_value_expr(
            ident,
            &related_type,
            &fk_lit,
            is_option,
        ));
        out.to_params_exprs.push(relation::generate_to_params_expr(
            ident,
            &related_type,
            &fk_lit,
            &fieldName,
            is_option,
        ));

        /* FK columns are real columns, so they can be indexed */
        out.valid_columns.insert(fk_col_name);
        out.relations.push(RelationInfo {
            col_name_lit: fk_lit,
            related_type,
            is_option,
        });
        return Ok(());
    }

    /* plain scalar field */
    let col_name: &str = attrs.name.as_deref().unwrap_or(&fieldName);
    let col_name_lit: syn::LitStr = syn::LitStr::new(col_name, ident.span());
    out.valid_columns.insert(col_name.to_string());

    out.col_exprs.push(columns::generate_column_expr(
        &attrs,
        col_name,
        active_type,
        is_option,
    )?);
    out.get_values_exprs.push(columns::generate_get_value_expr(
        ident,
        active_type,
        &col_name_lit,
    )?);
    out.to_params_exprs
        .push(columns::generate_to_params_expr(ident, &col_name_lit));
    Ok(())
}
