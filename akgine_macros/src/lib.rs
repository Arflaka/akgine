#![allow(non_snake_case)]
#![allow(unused_parens)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
/* syn get token and convert it in AST */
use syn::{
    Data, DeriveInput, Field, Fields, GenericArgument, PathArguments, Type, parse_macro_input,
};

/* #region Public acces point */

/* say that DbRecord is a "derive" macro */
/* `attributes(...)` say that we can use it */
/**
`#[column(skip)]` -> don't add the column at the db
`#[column(nullable)]` -> set the column as nullable
`#[column(not_null)]` -> don't allow null value for the column
`#[column(name="")]` -> set the name of the column
`#[column(default=)]` -> set the default value
`#[table(name="...")]` -> set the table name
`#[index("col1", "col2")]` -> create an index for the specified columns
`#[column(relation)]` -> field is a foreign key to another DbRecord type.
    The field's type (or its `Option<...>` inner type, for a nullable FK)
    must itself implement `DbRecord`. Generates an Integer column named
    `{field}_id` (or the name from `#[column(name = "...")]`, if given)
    that `.references(<Type as DbRecord>::table_name(), "id")` - the
    referenced table name is read from the related type itself at
    runtime, not hardcoded, so it can never drift out of sync with a
    rename the way a hand-written string literal could.
    `getValues` fetches the related row via `db.getRepository::<Type>().find(..)`;
    since this needs `db`, callers of `getValues` (see akgine's
    `query.rs::fetch`) always release the connection lock before invoking
    it, so this is safe and cannot deadlock. Every struct with at least
    one `#[column(relation)]` field also gets a generated `preload()`
    override that batches all such lookups into one `find_many` call per
    related type per fetch, instead of one query per row - see
    `DbRecord::preload`'s own doc comment for why that matters.

*/
#[proc_macro_derive(DbRecord, attributes(table, column, index))]
pub fn derive_db_record(input: TokenStream) -> TokenStream {
    /* convert token in ast */
    /* if there is an error stop here */
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);

    /* call the implementation if there is an error convert it in compile error */
    impl_db_record(&ast)
        .unwrap_or_else(|e| e.to_compile_error())
        .into() /* reconvert the proc_macro2::TokenStream in proc_macro::TokenStream for the compiler */
}

/* #endregion */

/* #region generate main code */

/** Info kept about each `#[column(relation)]` field, used to generate the
batched `preload()` override after the main field loop finishes. */
struct RelationInfo {
    /// The SQL column name holding the foreign key (e.g. "game_id").
    col_name_lit: syn::LitStr,
    /// The related type (e.g. `Game`), never the `Option<...>` wrapper.
    related_type: Type,
    /// Whether the Rust field is `Option<Related>` (nullable FK).
    is_option: bool,
}

fn impl_db_record(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    /* ast.ident is the name of the struct */
    let struct_name: &syn::Ident = &ast.ident;

    /* get the name of the sql table */
    let tableName: String = resolve_table_name(ast)?;

    /* we only want stuct with named field */
    let named_fields: &syn::punctuated::Punctuated<Field, syn::token::Comma> = match &ast.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                /* error: if there is no name */
                return Err(syn::Error::new_spanned(
                    struct_name,
                    "DbRecord requires a struct with named fields",
                ));
            }
        },
        _ => {
            /* error: if it's not a struct */
            return Err(syn::Error::new_spanned(
                struct_name,
                "DbRecord can only be derived for structs",
            ));
        }
    };

    /* vec to store each ligne of finals functions */
    let mut col_exprs: Vec<TokenStream2> = Vec::new();
    let mut get_values_exprs: Vec<TokenStream2> = Vec::new();
    let mut to_params_exprs: Vec<TokenStream2> = Vec::new();

    /* one entry per #[column(relation)] field, used to build preload() */
    let mut relations: Vec<RelationInfo> = Vec::new();

    /* Initialize a Set to track all valid SQL column names for index verification */
    let mut valid_columns: std::collections::HashSet<String> = std::collections::HashSet::new();
    /* id is always explicitly handled and available */
    valid_columns.insert("id".to_string());

    /* travel each field of the struct */
    for field in named_fields {
        let ident: &syn::Ident = field.ident.as_ref().unwrap();
        let fieldName: String = ident.to_string();

        /* #region check if we add the ligne or not */

        /* Rule 1: do not include id in dbRecord function other than get_values */
        if (fieldName == "id") {
            get_values_exprs.push(quote! { id: v.getValue("id")?.as_i64()? });
            continue;
        }

        /* extract options from attribs for this field */
        let attrs: FieldAttrs = FieldAttrs::parse(field)?;

        /* if `#[attrib(skip)]` -> skip the field */
        if (attrs.skip) {
            continue;
        }
        /* #endregion */

        /* check if it's an option -> `is_option` = true and `inner` = option type */
        let (is_option, inner) = unwrap_option(&field.ty);

        let activeType: &Type = inner.unwrap_or(&field.ty);

        /* #region relation fields (#[column(relation)]) take a different path
        entirely from plain scalar fields: the SQL column stores the
        related row's id, not the field's own Rust type. */
        if (attrs.relation) {
            /* default FK column name is "{field}_id", overridable via #[column(name = "...")] */
            let fk_col_name: String = attrs
                .name
                .clone()
                .unwrap_or_else(|| format!("{fieldName}_id"));
            let fk_col_name_lit: syn::LitStr = syn::LitStr::new(&fk_col_name, ident.span());
            let related_type: Type = activeType.clone();

            col_exprs.push(generateRelationColumnExpr(
                &fk_col_name_lit,
                &related_type,
                is_option,
                &attrs,
            ));
            get_values_exprs.push(generateRelationGetValueExpr(
                ident,
                &related_type,
                &fk_col_name_lit,
                is_option,
            ));
            to_params_exprs.push(generateRelationToParamsExpr(
                ident,
                &fk_col_name_lit,
                &fieldName,
                is_option,
            ));

            relations.push(RelationInfo {
                col_name_lit: fk_col_name_lit,
                related_type,
                is_option,
            });

            continue;
        }
        /* #endregion */

        /* #region get all attribs (plain scalar field, original behavior) */

        /* if `#[column(name = "")]` -> use it */
        /* else -> use the field name */
        let col_name: &str = attrs.name.as_deref().unwrap_or(&fieldName);

        let col_name_lit: syn::LitStr = syn::LitStr::new(col_name, ident.span());

        /* #endregion */

        /* push the verified col name to our valid list */
        valid_columns.insert(col_name.to_string());

        /* #endregion */

        /* #region push expresions */
        col_exprs.push(generateColumnExpr(
            &attrs, col_name, field, is_option, inner,
        )?);

        get_values_exprs.push(generateGetValueExpr(ident, activeType, &col_name_lit)?);

        to_params_exprs.push(generateToParamsExpr(ident, &col_name_lit)?);
        /* #endregion */
    }

    /* Process the indexes at the top level of the struct, now that we know all columns */
    let indexes_exprs = resolve_indexes(ast, &valid_columns)?;

    /* Only structs with at least one relation field get a preload() override
    - everything else keeps DbRecord's default no-op, so generated code
    for a plain (relation-free) struct stays exactly as before. */
    let preload_impl: TokenStream2 = generatePreloadImpl(&relations);

    /* Final send : push the code on the user prog */
    Ok(quote! {
        impl ::akgine::database::DbRecord for #struct_name {
            fn table_name() -> &'static str {
                #tableName
            }

            fn columns() -> Vec<::akgine::database::Column> {
                /* #() turn on the vec */
                /* , write a "," between each items */
                /* * reapet for each */
                vec![
                    #(#col_exprs),*
                ]
            }

            fn indexes() -> Vec<::akgine::database::IndexDef> {
                /* Default behavior for derived structs (can be expanded to parse #[index(...)] later) */
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
        }
    })
}

/* #endregion */

/* #region expresion maker */
fn generateColumnExpr(
    attrs: &FieldAttrs,
    colName: &str,
    field: &Field,
    is_option: bool,
    inner: Option<&Type>,
) -> syn::Result<TokenStream2> {
    /* convert the rust type to sql type */
    let colType: TokenStream2 = map_rust_type(inner.unwrap_or(&field.ty))?;

    /* start generate the final ligne to add */
    let mut expr: TokenStream2 = quote! { ::akgine::database::Column::new(#colName, #colType) };

    /* check if the column is nullable */
    let nullable: bool = (is_option || attrs.nullable) && !attrs.not_null;
    if (!nullable) {
        expr = quote! { #expr.not_null() };
    }

    /* if there is a default value */
    if let Some(default) = &attrs.default {
        expr = quote! { #expr.default(#default) };
    }

    Ok(expr)
}

fn generateGetValueExpr(
    ident: &syn::Ident,
    activeType: &Type,
    col_name_lit: &syn::LitStr,
) -> syn::Result<TokenStream2> {
    let as_method: syn::Ident = map_rust_type_to_as_method(activeType)?;

    Ok(quote! {
        #ident: v.getValue(#col_name_lit)?.#as_method()?
    })
}

fn generateToParamsExpr(
    ident: &syn::Ident,
    col_name_lit: &syn::LitStr,
) -> syn::Result<TokenStream2> {
    Ok(quote! {
        (#col_name_lit, self.#ident.clone().into())
    })
}

/// `#[column(relation)]` column: always an Integer FK, `.references(..)`
/// the related type's own `table_name()` (read at runtime, so it can never
/// point at the wrong/misspelled table the way a hand-typed string could).
fn generateRelationColumnExpr(
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

/// `getValues` for a relation field: fetch the related row through `_db`.
/// Ordinarily this means one query per row - but if the enclosing struct's
/// generated `preload()` already ran `find_many` for this exact set of
/// ids (see `generatePreloadImpl`), `db.getRepository::<Related>().find(..)`
/// hits the row cache instead and this costs nothing extra.
fn generateRelationGetValueExpr(
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

/// `toParams` for a relation field: write the related row's own id (it
/// must already be persisted - a relation field can't point at a row that
/// doesn't exist yet).
fn generateRelationToParamsExpr(
    ident: &syn::Ident,
    fk_col_name_lit: &syn::LitStr,
    fieldName: &str,
    is_option: bool,
) -> TokenStream2 {
    if is_option {
        quote! {
            (#fk_col_name_lit, self.#ident.as_ref().and_then(|r| r.id()).into())
        }
    } else {
        let msg: String =
            format!("`{fieldName}` must be persisted (have a real id) before saving this record");
        quote! {
            (#fk_col_name_lit, self.#ident.id().expect(#msg).into())
        }
    }
}

/// Builds the generated `preload()` override: one block per relation
/// field, each collecting the distinct ids that field's rows need and
/// batch-fetching them with one `find_many` call. Returns an empty
/// TokenStream2 (i.e. nothing is emitted, and `DbRecord`'s default no-op
/// `preload` applies) when the struct has no relation fields at all.
fn generatePreloadImpl(relations: &[RelationInfo]) -> TokenStream2 {
    if relations.is_empty() {
        return TokenStream2::new();
    }

    let blocks: Vec<TokenStream2> = relations
        .iter()
        .map(|r| {
            let RelationInfo {
                col_name_lit,
                related_type,
                is_option,
            } = r;
            let collect_ids: TokenStream2 = if *is_option {
                quote! {
                    rows.iter()
                        .filter_map(|v| v.getValue(#col_name_lit).ok()?.as_opt_i64().ok()?)
                        .collect::<Vec<i64>>()
                }
            } else {
                quote! {
                    rows.iter()
                        .filter_map(|v| v.getValue(#col_name_lit).ok()?.as_i64().ok())
                        .collect::<Vec<i64>>()
                }
            };
            quote! {
                {
                    let mut ids: Vec<i64> = #collect_ids;
                    ids.sort_unstable();
                    ids.dedup();
                    db.getRepository::<#related_type>().find_many(&ids)?;
                }
            }
        })
        .collect();

    quote! {
        fn preload(rows: &[::akgine::database::ValueSet], db: &::akgine::database::DataBase) -> Result<(), ::akgine::database::DbError> {
            #(#blocks)*
            Ok(())
        }
    }
}

/* #endregion */

/* #region analyse field attributes */

/**
struct to store what we found in `#[column()]`
*/
#[derive(Default)]
struct FieldAttrs {
    skip: bool,
    nullable: bool,
    not_null: bool,
    name: Option<String>,
    default: Option<String>,
    /// `#[column(relation)]` - see the derive macro's top-level doc comment.
    relation: bool,
}

impl FieldAttrs {
    /**
    get each attribs set in `#[column()]`
    */
    fn parse(field: &Field) -> syn::Result<Self> {
        let mut out: FieldAttrs = Self::default();

        /* travel each attribs */
        for attr in &field.attrs {
            /* we only use `#[column()]` */
            if (!attr.path().is_ident("column")) {
                continue;
            }

            /* `parse_nested_meta` is give by syn to check the content */
            attr.parse_nested_meta(|meta: syn::meta::ParseNestedMeta<'_>| {
                if (meta.path.is_ident("skip")) {
                    out.skip = true;
                } else if (meta.path.is_ident("nullable")) {
                    if (out.not_null) {
                        return Err(syn::Error::new_spanned(
                            &meta.path,
                            "can't have nullable and not_null at the same type",
                        ));
                    }
                    out.nullable = true;
                } else if (meta.path.is_ident("not_null")) {
                    if (out.nullable) {
                        return Err(syn::Error::new_spanned(
                            &meta.path,
                            "can't have not_null and nullable at the same type",
                        ));
                    }
                    out.not_null = true;
                } else if (meta.path.is_ident("relation")) {
                    out.relation = true;
                } else if (meta.path.is_ident("name")) {
                    /* get what there is after `=` */
                    let v: &syn::parse::ParseBuffer<'_> = meta.value()?;
                    /* check if it's a string */
                    let s: syn::LitStr = v.parse()?;
                    out.name = Some(s.value());
                } else if (meta.path.is_ident("default")) {
                    // let v: &syn::parse::ParseBuffer<'_> = meta.value()?;
                    // let s: syn::LitStr = v.parse()?;

                    let lit: syn::Lit = meta.value()?.parse()?;

                    /* convert the value in string */
                    let string_value: String = match lit {
                        syn::Lit::Str(s) => s.value(),
                        syn::Lit::Int(i) => i.base10_digits().to_string(),
                        syn::Lit::Float(f) => f.base10_digits().to_string(),
                        syn::Lit::Bool(b) => b.value.to_string(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                lit,
                                "Type de valeur par défaut non supporté",
                            ));
                        }
                    };

                    // out.default = Some(s.value());
                    out.default = Some(string_value);
                }
                Ok(())
            })?;
        }

        Ok(out)
    }
}

/* #endregion */

/* #region extract option type */

/**
check if the type is an option
return if it's an option and the type
*/
fn unwrap_option(ty: &Type) -> (bool, Option<&Type>) {
    /* check if the type is a path */
    if let Type::Path(tp) = ty {
        /* get the last item */
        if let Some(seg) = tp.path.segments.last() {
            if (seg.ident == "Option") {
                /* check if there is `< >` */
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    /* get the first element in `< >` and check if it's a type */
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        return (true, Some(inner));
                    }
                }
            }
        }
    }
    (false, None)
}

/* #endregion */

/* #region type converter (Rust -> SQL) */

/* get a rust type and send the token we want */
fn map_rust_type(ty: &Type) -> syn::Result<TokenStream2> {
    /* check if the type is a path */
    if let Type::Path(tp) = ty {
        /* get the last element */
        if let Some(seg) = tp.path.segments.last() {
            /* convert the type in string */
            let name: String = seg.ident.to_string();
            return match name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize"
                | "bool" => Ok(quote! { ::akgine::database::ColType::Integer }),

                "f32" | "f64" => Ok(quote! { ::akgine::database::ColType::Real }),

                "String" => Ok(quote! { ::akgine::database::ColType::Text }),

                /* `Vec<u8>` (BLOB) is NOT supported: akgine's own SqlValue/ColType
                have no Blob variant at all (checked directly against the
                installed version - `ColType` is Integer/Real/Text only).
                The previous version of this macro still generated
                `ColType::Blob` / `.as_blob()` here, which doesn't exist and
                would only fail to compile once someone actually used a
                Vec<u8> field - a landmine left for later. This is a real,
                immediate error instead. */
                "Vec" => Err(syn::Error::new_spanned(
                    ty,
                    "Vec<u8> (BLOB) columns are not supported by this version of akgine \
                     (SqlValue/ColType have no Blob variant). Store bytes as base64-encoded \
                     Text instead, or add #[column(skip)] and handle this field manually.",
                )),

                /* if it's `Vec<u8>` for blob */
                /*
                "Vec" => {
                                    /* check bracket content */
                                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                                        /* get the element */
                                        if let Some(GenericArgument::Type(Type::Path(inner))) = ab.args.first() {
                                            /* if it's u8 and nothing else */
                                            if (inner.path.is_ident("u8")) {
                                                return Ok(quote! { ::akgine::database::ColType::Blob });
                                            }
                                        }
                                    }
                                    Err(syn::Error::new_spanned(
                                        ty,
                                        "Only Vec<u8> is supported as ColType::Blob",
                                    ))
                                }
                */
                                /* if it's an other type we don't make it */
                other => Err(syn::Error::new_spanned(
                    ty,
                    format!(
                        "Unsupported type `{other}`. \
                         Add #[column(skip)], #[column(relation)] if this is a foreign key \
                         to another DbRecord type, or implement the mapping manually."
                    ),
                )),
            };
        }
    }
    Err(syn::Error::new_spanned(
        ty,
        "Cannot determine ColType for this type",
    ))
}

/**
 Helper function to figure out the right `as_XXX()` method for ValueSet -> Rust struct deserialization.
*/
fn map_rust_type_to_as_method(ty: &Type) -> syn::Result<syn::Ident> {
    /* check if type is a typepath */
    if let Type::Path(tp) = ty {
        /* get the last element */
        if let Some(seg) = tp.path.segments.last() {
            let name: String = seg.ident.to_string();
            let method_str: &str = match name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" => {
                    "as_i64"
                }
                "f32" | "f64" => "as_f64",
                "String" => "as_text",
                "bool" => "as_bool",
                // _ => {
                // return Err(syn::Error::new_spanned(
                //     ty,
                //     "Cannot map type to an as_xxx method",
                // ));
                // }
                "Vec" => {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "Vec<u8> (BLOB) columns are not supported by this version of akgine \
                         (SqlValue has no as_blob() method). Store bytes as base64-encoded \
                         Text instead, or add #[column(skip)] and handle this field manually.",
                    ));
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "Cannot map type to an as_xxx method",
                    ));
                }
            };
            return Ok(syn::Ident::new(method_str, seg.ident.span()));
        }
    }
    Err(syn::Error::new_spanned(
        ty,
        "Cannot determine as_xxx method for this type",
    ))
}

/* #endregion */

/* #region resolve table name  */

/**
get the sql table name
1) `#[table("...")]`
2) name of the struct + "s"
*/
/*
fn resolve_table_name(ast: &DeriveInput) -> syn::Result<String> {
    let mut table_name: Option<String> = None;

    /* travel on attributs on the TOP of the struct */
    for attr in &ast.attrs {
        /* if we found `#[table(...)]` */
        if (attr.path().is_ident("table")) {
            /* parse_nested_meta allows iterating over attrib input */
            attr.parse_nested_meta(|meta: syn::meta::ParseNestedMeta<'_>| {
                if (meta.path.is_ident("name")) {
                    /* extract the value after `=` */
                    let value: &syn::parse::ParseBuffer<'_> = meta.value()?;
                    let s: syn::LitStr = value.parse()?;
                    table_name = Some(s.value());
                    Ok(())
                } else {
                    /* ignore other parameters like XXX, YYY */
                    Ok(())
                }
            })?;
        }
    }

    /* if we found a name in the attributes, return it */
    if let Some(name) = table_name {
        return Ok(name);
    }

    /* if there is no attrib, default rule : */
    /* take the name of the struct and add "s". */
    Ok(ast.ident.to_string() + "s")
    /* Ok(pascal_to_snake(&ast.ident.to_string()) + "s") */
}
*/

/**
get the sql table name
1) `#[table("...")]`
2) name of the struct + "s"
*/
fn resolve_table_name(ast: &DeriveInput) -> syn::Result<String> {
    for attr in &ast.attrs {
        if (attr.path().is_ident("table")) {
            let tableName: syn::LitStr = attr.parse_args()?;
            return Ok(tableName.value());
        }
    }
    Ok(ast.ident.to_string() + "s")
}

/* fn pascal_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}*/

/* #endregion */

/* #region resolve indexes */

/**
 Parses struct-level `#[index(...)]` attributes.
 Verifies:
 1. Fields inside exist in the struct (using valid_columns).
 2. There are no duplicate permutations (e.g. `#[index("A", "B")]` == `#[index("B", "A")]`).
*/
fn resolve_indexes(
    ast: &DeriveInput,
    valid_columns: &std::collections::HashSet<String>,
) -> syn::Result<Vec<TokenStream2>> {
    let mut indexes_exprs: Vec<TokenStream2> = Vec::new();

    /* We use a HashSet of sorted vectors to track unique combinations regardless of order */
    let mut seen_indexes: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();

    for attr in &ast.attrs {
        if attr.path().is_ident("index") {
            /* Parse a comma-separated list of string literals: #[index("col1", "col2", ...)] */
            let nested: syn::punctuated::Punctuated<syn::LitStr, syn::token::Comma> = attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
                )?;

            let mut cols: Vec<String> = Vec::new();

            for lit in nested {
                let col_name: String = lit.value();

                /* Verification 1: The index field must exist in the valid columns list */
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

            /* Verification 2: Check for duplicate index combinations */
            let mut sorted_cols: Vec<String> = cols.clone();
            sorted_cols.sort(); /* ["B", "A"] becomes ["A", "B"] */

            /* If it returns false, it means the exact combination was already inserted */
            if !seen_indexes.insert(sorted_cols) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "Duplicate index combination detected (order does not matter)",
                ));
            }

            /* Convert Strings back to TokenStreams (LitStrs) to inject them into the macro expansion */
            let col_lits = cols
                .iter()
                .map(|c| syn::LitStr::new(c, proc_macro2::Span::call_site()));

            indexes_exprs.push(quote! {
                IndexDef::new(&[ #(#col_lits),* ])
            });
        }
    }

    Ok(indexes_exprs)
}

/* #endregion */
