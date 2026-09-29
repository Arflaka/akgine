/*! Resolves the SQL table name. */
use proc_macro2::TokenStream;
use syn::{DeriveInput, Expr, Ident, LitStr, Token, parse::ParseStream};

/**
 * Resolves the table name of a struct.
 *
 *  Supported forms (parameters can be mixed and in any order):
 *  - `#[table("users")]`
 *  - `#[table(name = "users")]`
 *  - `#[table(schema = "public", name = "users")]`
 *  - `#[table("users", schema = "public")]`
 *
 *  Falls back to the struct name + "s" when no name is found.
 */
pub(super) fn resolve_table_name(ast: &DeriveInput) -> syn::Result<String> {
    /* attributes are read lazily: the search stops at the first `#[table]`
    that contains a name, and the attributes after it are never read */
    let table_name: Option<LitStr> = ast
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("table"))
        .map(|attr| attr.parse_args_with(parse_table_name))
        /* takes each element, applies a function, and stops at the first result that is `Some` */
        .find_map(Result::transpose)
        /* get of the Option<Resulut> in Result */
        .transpose()?;

    /* check if there is a name or create it */
    Ok(table_name.map_or_else(|| format!("{}s", ast.ident), |name| name.value()))
}

/**
 *  Reads the parameters one by one and stops as soon as the table name is found.
 */
fn parse_table_name(input: ParseStream) -> syn::Result<Option<LitStr>> {
    let mut table_name: Option<LitStr> = None;

    /* keep looping only while the name is NOT found AND there are parameters left */
    while (table_name.is_none() && !input.is_empty()) {
        table_name = parse_param_name(input)?;

        /* a comma is required between parameters, but optional after the last one */
        if (table_name.is_none() && !input.is_empty()) {
            input.parse::<Token![,]>()?;
        }
    }

    /* `syn` requires the parser to consume every token, so the parameters
    after the name are skipped here as raw tokens, without being interpreted */
    input.parse::<TokenStream>()?;

    Ok(table_name)
}

/**
 * Parses a single parameter and returns its value only if it is the table name.
 */
fn parse_param_name(input: ParseStream) -> syn::Result<Option<LitStr>> {
    /* positional form: `"users"` */
    if (input.peek(LitStr)) {
        return input.parse().map(Some);
    }

    let key: Ident = input.parse()?;

    /* flag without value (e.g. `unique`): nothing to read */
    if (!input.peek(Token![=])) {
        return Ok(None);
    }
    input.parse::<Token![=]>()?;

    if (key == "name") {
        /* `name = "users"` */
        input.parse().map(Some)
    } else {
        /* unrelated `key = value`: consume the value and ignore it */
        input.parse::<Expr>()?;
        Ok(None)
    }
}
