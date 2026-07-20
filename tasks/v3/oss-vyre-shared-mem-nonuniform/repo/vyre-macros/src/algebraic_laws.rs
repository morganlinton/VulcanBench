use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, LitStr, Token};

/// Derive `vyre::AlgebraicLawProvider` from a `#[vyre(laws = [...])]` attribute.
///
/// Attach the derive to a unit struct (or any struct) that represents an op
/// type. List its algebraic laws in the attribute; the macro emits the trait
/// impl plus a `const LAWS: &[AlgebraicLaw]` associated item.
///
/// # Example
///
/// ```ignore
/// use vyre_macros::AlgebraicLaws;
///
/// #[derive(AlgebraicLaws)]
/// #[vyre(laws = [Commutative, Associative, "Identity { element: 0 }"])]
/// pub struct Xor;
/// ```
///
/// Expands to:
///
/// ```ignore
/// impl Xor {
///     pub const LAWS: &'static [::vyre::ops::AlgebraicLaw] = &[
///         ::vyre::ops::AlgebraicLaw::Commutative,
///         ::vyre::ops::AlgebraicLaw::Associative,
///         ::vyre::ops::AlgebraicLaw::Identity { element: 0 },
///     ];
/// }
/// impl ::vyre::ops::AlgebraicLawProvider for Xor {
///     fn laws() -> &'static [::vyre::ops::AlgebraicLaw] { Self::LAWS }
/// }
/// ```
pub(crate) fn derive_algebraic_laws_impl(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;
    let laws = match extract_laws_attribute(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    // Parse each law string as an AlgebraicLaw variant expression.
    let law_exprs = laws.iter().map(|lit| {
        let src = lit.value();
        let trimmed = src.trim();
        let path: syn::Expr = match syn::parse_str(&format!("::vyre::ops::AlgebraicLaw::{trimmed}"))
        {
            Ok(e) => e,
            Err(err) => {
                return syn::Error::new_spanned(
                    lit,
                    format!("failed to parse AlgebraicLaw variant `{trimmed}`: {err}"),
                )
                .to_compile_error();
            }
        };
        quote! { #path }
    });

    // ensure the input type is a struct/enum we can attach impls to
    match &input.data {
        Data::Struct(_) | Data::Enum(_) => {}
        Data::Union(_) => {
            return syn::Error::new_spanned(
                ident,
                "#[derive(AlgebraicLaws)] does not support unions. Fix: derive it on the op struct or enum that declares algebraic laws.",
            )
            .to_compile_error()
            .into();
        }
    }

    let law_exprs_vec: Vec<_> = law_exprs.collect();

    quote! {
        impl #ident {
            /// Algebraic laws declared on this op type.
            pub const LAWS: &'static [::vyre::ops::AlgebraicLaw] = &[
                #(#law_exprs_vec),*
            ];
        }

        impl ::vyre::ops::AlgebraicLawProvider for #ident {
            fn laws() -> &'static [::vyre::ops::AlgebraicLaw] {
                Self::LAWS
            }
        }
    }
    .into()
}
pub(crate) fn extract_laws_attribute(attrs: &[Attribute]) -> syn::Result<Vec<LitStr>> {
    for attr in attrs {
        if !attr.path().is_ident("vyre") {
            continue;
        }
        let mut laws: Option<Vec<LitStr>> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("laws") {
                let value = meta.value()?;
                // Accept both [Commutative, Identity{element:0}] bracketed
                // identifier lists and [ "Commutative", "Identity{element:0}" ]
                // string-literal arrays.
                let lookahead = value.lookahead1();
                if lookahead.peek(syn::token::Bracket) {
                    let content;
                    syn::bracketed!(content in value);
                    let mut collected = Vec::new();
                    while !content.is_empty() {
                        if content.peek(LitStr) {
                            let lit: LitStr = content.parse()?;
                            collected.push(lit);
                        } else {
                            // parse as raw token stream up to the next comma
                            let expr: syn::Expr = content.parse()?;
                            let rendered = quote! { #expr }.to_string();
                            collected.push(LitStr::new(&rendered, expr.span()));
                        }
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    laws = Some(collected);
                    Ok(())
                } else {
                    Err(meta.error("expected `laws = [..]`"))
                }
            } else {
                Err(meta.error("unknown vyre() argument; expected `laws = [..]`"))
            }
        })?;
        if let Some(l) = laws {
            return Ok(l);
        }
    }
    Ok(Vec::new())
}
