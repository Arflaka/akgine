/*!
 * Rust type -> SQL type mapping, and Rust type -> `as_xxx()` accessor.
 * To support a new Rust type, edit both functions here.
 */

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type;

const BLOB_MSG: &str = "Vec<u8> (BLOB) columns are not supported by this version of akgine \
     (SqlValue/ColType have no Blob variant). Store bytes as base64-encoded \
     Text instead, or add #[column(skip)] and handle this field manually.";

/**
 * Returns the `ColType` token for a Rust type.
 */
pub(super) fn map_rust_type(ty: &Type) -> syn::Result<TokenStream2> {
    /* `Type::Path` covers named types like `i32` or `std::string::String` */
    if let Type::Path(tp) = ty {
        /* unwrap the lastest part of the typePath */
        if let Some(seg) = tp.path.segments.last() {
            /* convert the identifier into a string to compare it */
            let name: String = seg.ident.to_string();
            return match name.as_str() {
                /* All integer types (and `bool`, stored as 0/1) map to SQL INTEGER */
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize"
                | "bool" => Ok(quote! { ::akgine::database::ColType::Integer }),
                "f32" | "f64" => Ok(quote! { ::akgine::database::ColType::Real }),
                "String" => Ok(quote! { ::akgine::database::ColType::Text }),
                /* akgine has no Blob variant: fail now rather than at use time */
                /* `new_spanned(ty, msg)` creates a compile error underlining `ty` in the user's code. */
                "Vec" => Err(syn::Error::new_spanned(ty, BLOB_MSG)),
                /* `other` catches any remaining name and binds it to a variable. */
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
    /* Reached if the type is not a path (e.g. a reference or a tuple) or has no segments. */
    Err(syn::Error::new_spanned(
        ty,
        "Cannot determine ColType for this type",
    ))
}

/**
 *  Returns the `as_xxx` method used to read a `ValueSet` value into this type.
 */
pub(super) fn map_rust_type_to_as_method(ty: &Type) -> syn::Result<syn::Ident> {
    /* `Type::Path` covers named types like `i32` or `std::string::String` */
    if let Type::Path(tp) = ty {
        /* unwrap the lastest part of the typePath */
        if let Some(seg) = tp.path.segments.last() {
            /* convert the identifier into a string to compare it */
            let name: String = seg.ident.to_string();

            let method_str: &str = match name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" => {
                    "as_i64"
                }
                "f32" | "f64" => "as_f64",
                "String" => "as_text",
                "bool" => "as_bool",
                "Vec" => return Err(syn::Error::new_spanned(ty, BLOB_MSG)),
                _ => {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "Cannot map type to an as_xxx method",
                    ));
                }
            };
            /* Build an identifier from the string.
            `seg.ident.span()` makes any later error point at the user's field type. */
            return Ok(syn::Ident::new(method_str, seg.ident.span()));
        }
    }
    /* Fallback error for non-path types. */
    Err(syn::Error::new_spanned(
        ty,
        "Cannot determine as_xxx method for this type",
    ))
}
