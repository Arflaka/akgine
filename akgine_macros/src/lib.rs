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
/* `attributes(table, column)` say that we can use it */
/**
`#[column(skip)]` -> don't add the column at the db
`#[column(nullable)]` -> set the column as nullable
`#[column(not_null)]` -> don't allow null value for the column
`#[column(name="")]` -> set the name of the column
`#[column(default=)]` -> set the default value
*/
#[proc_macro_derive(DbRecord, attributes(table, column))]
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

        /* #region get all attribs */

        /* if `#[column(name = "")]` -> use it */
        /* else -> use the field name */
        let col_name: &str = attrs.name.as_deref().unwrap_or(&fieldName);


        /* check if it's an option -> `is_option` = true and `inner` = option type */
        let (is_option, inner) = unwrap_option(&field.ty);

        let activeType: &Type = inner.unwrap_or(&field.ty);
        
        let col_name_lit: syn::LitStr = syn::LitStr::new(col_name, ident.span());

        /* #endregion */

        /* #region push expresions */
        col_exprs.push(generateColumnExpr(&attrs, col_name, field, is_option, inner)?);
        
        get_values_exprs.push(generateGetValueExpr(ident, activeType, &col_name_lit)?);

        to_params_exprs.push(generateToParamsExpr(ident, &col_name_lit)?);
        /* #endregion */
    }

    /* Final send : push the code on the user prog */
    Ok(quote! {
        impl DbRecord for #struct_name {
            fn table_name() -> &'static str {
                #tableName
            }

            fn columns() -> Vec<Column> {
                /* #() turn on the vec */
                /* , write a "," between each items */
                /* * reapet for each */
                vec![ #(#col_exprs),* ]
            }

            fn indexes() -> Vec<IndexDef> {
                /* Default behavior for derived structs (can be expanded to parse #[index(...)] later) */
                vec![]
            }

            fn getValues(v: &ValueSet) -> Result<Self, DbError> {
                Ok(Self {
                    #(#get_values_exprs),*
                })
            }

            fn toParams(&self) -> Vec<(&'static str, SqlValue)> {
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
fn generateColumnExpr(attrs: &FieldAttrs, colName: &str, field: &Field, is_option: bool, inner: Option<&Type>) -> syn::Result<TokenStream2> {
        /* convert the rust type to sql type */
        let colType:TokenStream2  = map_rust_type(inner.unwrap_or(&field.ty))?;


        /* start generate the final ligne to add */
        let mut expr: TokenStream2 = quote! { Column::new(#colName, #colType) };

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

fn generateGetValueExpr(ident: &syn::Ident, activeType: &Type, col_name_lit: &syn::LitStr) -> syn::Result<TokenStream2> {
        let as_method: syn::Ident = map_rust_type_to_as_method(activeType)?;

        Ok(quote! {
            #ident: v.getValue(#col_name_lit)?.#as_method()?
        })
}

fn generateToParamsExpr(ident: &syn::Ident, col_name_lit: &syn::LitStr) -> syn::Result<TokenStream2> {
        Ok(quote! {
            (#col_name_lit, self.#ident.clone().into())
        })
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
                | "bool" => Ok(quote! { ColType::Integer }),

                "f32" | "f64" => Ok(quote! { ColType::Real }),

                "String" => Ok(quote! { ColType::Text }),

                /* if it's `Vec<u8>` for blob */
                "Vec" => {
                    /* check bracket content */
                    if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                        /* get the element */
                        if let Some(GenericArgument::Type(Type::Path(inner))) = ab.args.first() {
                            /* if it's u8 and nothing else */
                            if (inner.path.is_ident("u8")) {
                                return Ok(quote! { ColType::Blob });
                            }
                        }
                    }
                    Err(syn::Error::new_spanned(
                        ty,
                        "Only Vec<u8> is supported as ColType::Blob",
                    ))
                }

                /* if it's an other type we don't make it */
                other => Err(syn::Error::new_spanned(
                    ty,
                    format!(
                        "Unsupported type `{other}`. \
                         Add #[column(skip)] or implement the mapping manually."
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
                "Vec" => "as_blob",
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
                }
                else {
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
