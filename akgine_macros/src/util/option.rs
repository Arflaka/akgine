use syn::{GenericArgument, PathArguments, Type};

/**
 *  Checks whether `ty` is `Option<T>`.
 *  - Returns `(true, Some(T))` if so, `(false, None)` otherwise.
 */
pub fn unwrap_option(ty: &Type) -> (bool, Option<&Type>) {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if (seg.ident == "Option") {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        return (true, Some(inner));
                    }
                }
            }
        }
    }
    (false, None)
}
