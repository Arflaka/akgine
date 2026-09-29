//! Generates the batched `preload()` override for structs with relations.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::relation::RelationInfo;

/// One block per relation field: collect the distinct ids the rows need and
/// batch-fetch them with a single `find_many`. Returns an empty stream when
/// there are no relations, so `DbRecord`'s default no-op `preload` applies.
pub(super) fn generate_preload_impl(relations: &[RelationInfo]) -> TokenStream2 {
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
