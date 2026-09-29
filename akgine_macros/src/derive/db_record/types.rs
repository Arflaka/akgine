//! Rust type -> SQL type mapping, and Rust type -> `as_xxx()` accessor.
//! To support a new Rust type, edit both functions here.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type;

const BLOB_MSG: &str = "Vec<u8> (BLOB) columns are not supported by this version of akgine \
     (SqlValue/ColType have no Blob variant). Store bytes as base64-encoded \
     Text instead, or add #[column(skip)] and handle this field manually.";

/// Returns the `ColType` token for a Rust type.
pub(super) fn map_rust_type(ty: &Type) -> syn::Result<TokenStream2> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            let name: String = seg.ident.to_string();
            return match name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize"
                | "bool" => Ok(quote! { ::akgine::database::ColType::Integer }),
                "f32" | "f64" => Ok(quote! { ::akgine::database::ColType::Real }),
                "String" => Ok(quote! { ::akgine::database::ColType::Text }),
                /* akgine has no Blob variant: fail now rather than at use time */
                "Vec" => Err(syn::Error::new_spanned(ty, BLOB_MSG)),
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

/// Returns the `as_xxx` method used to read a `ValueSet` value into this type.
pub(super) fn map_rust_type_to_as_method(ty: &Type) -> syn::Result<syn::Ident> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
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
            return Ok(syn::Ident::new(method_str, seg.ident.span()));
        }
    }
    Err(syn::Error::new_spanned(
        ty,
        "Cannot determine as_xxx method for this type",
    ))
}
