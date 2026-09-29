/*!
 *  how to add a new derive
 *  1. Create `derive/my_derive/mod.rs` exposing
 *     `pub fn expand(ast: &DeriveInput) -> syn::Result<TokenStream2>`.
 *  2. Declare it below with `pub mod my_derive;`.
 *  3. Add a 3-line `#[proc_macro_derive]` function in `lib.rs`.
 */

pub mod db_record;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::{DeriveInput, parse_macro_input};

/**
 * Shared glue for every derive: parse tokens into an AST, run the
 * derive-specific `expand`, and turn any `syn::Error` into a compile error.
 */
pub fn run(
    input: TokenStream,
    expand: fn(&DeriveInput) -> syn::Result<TokenStream2>,
) -> TokenStream {
    /* on a parse error, this returns a compile error immediately */
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);

    expand(&ast).unwrap_or_else(|e| e.to_compile_error()).into() /* proc_macro2 -> proc_macro for the compiler */
}
