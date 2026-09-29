//! Public entry points of the `akgine` procedural macros.
//!
//! This file must stay tiny: it only *declares* each macro and forwards to
//! its implementation in `derive/`. No real logic lives here.
//!
//! To add a new derive, see `derive/mod.rs`.
#![allow(non_snake_case)]
#![allow(unused_parens)]

mod derive;
mod util;

use proc_macro::TokenStream;

/// Derive macro generating the `DbRecord` implementation.
///
/// Supported attributes:
/// - `#[column(skip)]`      -> no column; field is filled with `Default::default()`
/// - `#[column(nullable)]`  -> column is nullable
/// - `#[column(not_null)]`  -> column forbids null
/// - `#[column(name="")]`   -> custom column name
/// - `#[column(default=)]`  -> default value
/// - `#[column(relation)]`  -> foreign key to another `DbRecord` type
///   (see `derive/db_record/relation.rs` and `preload.rs`)
/// - `#[table(name="...")]` or `#[table("...")]` -> table name
/// - `#[index("col1", "col2")]` -> index on the given columns
#[proc_macro_derive(DbRecord, attributes(table, column, index))]
pub fn derive_db_record(input: TokenStream) -> TokenStream {
    derive::run(input, derive::db_record::expand)
}

// Add new derives here, one small function each, e.g.:
//
// #[proc_macro_derive(MyDerive, attributes(my_attr))]
// pub fn derive_my_derive(input: TokenStream) -> TokenStream {
//     derive::run(input, derive::my_derive::expand)
// }
