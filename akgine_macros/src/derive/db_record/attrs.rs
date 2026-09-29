//! Parsing of the field-level `#[column(...)]` attribute.

use syn::Field;

/// What we found in `#[column(...)]`.
#[derive(Default)]
pub(super) struct FieldAttrs {
    pub skip: bool,
    pub nullable: bool,
    pub not_null: bool,
    pub name: Option<String>,
    pub default: Option<String>,
    /// `#[column(relation)]` - the field is a foreign key to another DbRecord.
    pub relation: bool,
}

impl FieldAttrs {
    pub(super) fn parse(field: &Field) -> syn::Result<Self> {
        let mut out: FieldAttrs = Self::default();

        for attr in &field.attrs {
            /* we only care about `#[column(...)]` */
            if (!attr.path().is_ident("column")) {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if (meta.path.is_ident("skip")) {
                    out.skip = true;
                } else if (meta.path.is_ident("nullable")) {
                    if (out.not_null) {
                        return Err(syn::Error::new_spanned(
                            &meta.path,
                            "can't have nullable and not_null at the same time",
                        ));
                    }
                    out.nullable = true;
                } else if (meta.path.is_ident("not_null")) {
                    if (out.nullable) {
                        return Err(syn::Error::new_spanned(
                            &meta.path,
                            "can't have not_null and nullable at the same time",
                        ));
                    }
                    out.not_null = true;
                } else if (meta.path.is_ident("relation")) {
                    out.relation = true;
                } else if (meta.path.is_ident("name")) {
                    /* value after `=` must be a string literal */
                    let s: syn::LitStr = meta.value()?.parse()?;
                    out.name = Some(s.value());
                } else if (meta.path.is_ident("default")) {
                    let lit: syn::Lit = meta.value()?.parse()?;
                    let string_value: String = match lit {
                        syn::Lit::Str(s) => s.value(),
                        syn::Lit::Int(i) => i.base10_digits().to_string(),
                        syn::Lit::Float(f) => f.base10_digits().to_string(),
                        syn::Lit::Bool(b) => b.value.to_string(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                lit,
                                "Unsupported default value type",
                            ));
                        }
                    };
                    out.default = Some(string_value);
                }
                Ok(())
            })?;
        }

        Ok(out)
    }
}
