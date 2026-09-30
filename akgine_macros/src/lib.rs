#![allow(non_snake_case)]
#![allow(unused_parens)]

mod derive;
mod util;

use proc_macro::TokenStream;

/**
 * Derive macro generating the `DbRecord` implementation.
 *
 * Supported attributes:
 * - `#[table(name="...")]` or `#[table("...")]` -> table name
 * - `#[column(skip)]`      -> no column; field is filled with `Default::default()`
 * - `#[column(nullable)]`  -> column is nullable
 * - `#[column(not_null)]`  -> column forbids null
 * - `#[column(name="")]`   -> custom column name
 * - `#[column(default=)]`  -> default value
 * - `#[column(relation)]`  -> foreign key to another `DbRecord` type
 *   (see `derive/db_record/relation.rs` and `preload.rs`)
 * - `#[index("col1", "col2")]` -> index on the given columns
*/
#[proc_macro_derive(DbRecord, attributes(table, column, index))]
pub fn derive_db_record(input: TokenStream) -> TokenStream {
    derive::run(input, derive::db_record::expand)
}
