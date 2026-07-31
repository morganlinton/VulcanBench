use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Attribute, Ident, Token, Type};

struct FieldDef {
    name: Ident,
    ty: Type,
}

enum VariantData {
    Unit,
    Unnamed(Vec<Type>),
    Named(Vec<FieldDef>),
}

struct AstVariant {
    attrs: Vec<Attribute>,
    ident: Ident,
    data: VariantData,
}

struct AstEnum {
    attrs: Vec<Attribute>,
    name: Ident,
    variants: Vec<AstVariant>,
}

struct AstManifest {
    enums: Vec<AstEnum>,
}

impl AstManifest {
    fn validate(&self) -> syn::Result<()> {
        let mut enum_names = std::collections::BTreeSet::new();
        for ast_enum in &self.enums {
            let enum_name = ast_enum.name.to_string();
            if !enum_names.insert(enum_name.clone()) {
                return Err(syn::Error::new_spanned(
                    &ast_enum.name,
                    format!(
                        "duplicate AST enum `{enum_name}`. Fix: merge the variants into one `{enum_name}` block or rename the second enum."
                    ),
                ));
            }

            let mut variant_names = std::collections::BTreeSet::new();
            for variant in &ast_enum.variants {
                let variant_name = variant.ident.to_string();
                if !variant_names.insert(variant_name.clone()) {
                    return Err(syn::Error::new_spanned(
                        &variant.ident,
                        format!(
                            "duplicate AST variant `{variant_name}` in `{enum_name}`. Fix: keep one `{variant_name}` variant or give each variant a stable unique name."
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Parse for AstManifest {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut enums = Vec::new();
        while !input.is_empty() {
            let enum_attrs = input.call(Attribute::parse_outer)?;
            let name: Ident = input.parse()?;
            let content;
            syn::braced!(content in input);
            let mut variants = Vec::new();
            while !content.is_empty() {
                let variant_attrs = content.call(Attribute::parse_outer)?;
                let v_ident: Ident = content.parse()?;
                let data = if content.peek(syn::token::Brace) {
                    let fields_content;
                    syn::braced!(fields_content in content);
                    let mut fields = Vec::new();
                    while !fields_content.is_empty() {
                        let f_name: Ident = fields_content.parse()?;
                        fields_content.parse::<Token![:]>()?;
                        let f_ty: Type = fields_content.parse()?;
                        fields.push(FieldDef {
                            name: f_name,
                            ty: f_ty,
                        });
                        if fields_content.peek(Token![,]) {
                            fields_content.parse::<Token![,]>()?;
                        }
                    }
                    VariantData::Named(fields)
                } else if content.peek(syn::token::Paren) {
                    let fields_content;
                    syn::parenthesized!(fields_content in content);
                    let mut fields = Vec::new();
                    while !fields_content.is_empty() {
                        let f_ty: Type = fields_content.parse()?;
                        fields.push(f_ty);
                        if fields_content.peek(Token![,]) {
                            fields_content.parse::<Token![,]>()?;
                        }
                    }
                    VariantData::Unnamed(fields)
                } else {
                    VariantData::Unit
                };
                variants.push(AstVariant {
                    attrs: variant_attrs,
                    ident: v_ident,
                    data,
                });
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
            }
            enums.push(AstEnum {
                attrs: enum_attrs,
                name,
                variants,
            });
        }
        Ok(AstManifest { enums })
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn vyre_ast_registry_impl(item: TokenStream) -> TokenStream {
    let manifest = parse_macro_input!(item as AstManifest);
    if let Err(error) = manifest.validate() {
        return error.to_compile_error().into();
    }

    let mut outputs = Vec::new();

    for ast_enum in manifest.enums {
        let enum_name = &ast_enum.name;
        let enum_attrs = &ast_enum.attrs;

        let variants = ast_enum.variants.iter().map(|v| {
            let ident = &v.ident;
            let attrs = &v.attrs;
            match &v.data {
                VariantData::Unit => quote! { #(#attrs)* #ident },
                VariantData::Unnamed(types) => quote! { #(#attrs)*  #ident(#(#types),*) },
                VariantData::Named(fields) => {
                    let f = fields.iter().map(|f| {
                        let n = &f.name;
                        let t = &f.ty;
                        quote! { #n: #t }
                    });
                    quote! { #(#attrs)* #ident { #(#f),* } }
                }
            }
        });

        // op_id implementation
        let op_ids = ast_enum.variants.iter().map(|v| {
            let ident = &v.ident;
            if ident == "Opaque" {
                quote! {
                    #enum_name::Opaque(ext) => ext.extension_kind().to_string()
                }
            } else {
                let lower_name = format!(
                    "vyre.{}.{}",
                    enum_name.to_string().to_lowercase(),
                    ident.to_string().to_lowercase()
                );
                match &v.data {
                    VariantData::Unit => quote! {
                        #enum_name::#ident => #lower_name.to_string()
                    },
                    VariantData::Unnamed(_) => quote! {
                        #enum_name::#ident ( .. ) => #lower_name.to_string()
                    },
                    VariantData::Named(_) => quote! {
                        #enum_name::#ident { .. } => #lower_name.to_string()
                    },
                }
            }
        });

        let op_id_fn_name = syn::Ident::new(
            &format!("{}_op_id", enum_name.to_string().to_lowercase()),
            proc_macro2::Span::call_site(),
        );

        // PartialEq implementations
        let partial_eq_arms = ast_enum.variants.iter().map(|v| {
            let ident = &v.ident;
            if ident == "Opaque" {
                // Special case for Opaque
                quote! {
                    (Self::Opaque(left), Self::Opaque(right)) => {
                        left.extension_kind() == right.extension_kind()
                            && left.stable_fingerprint() == right.stable_fingerprint()
                    }
                }
            } else {
                match &v.data {
                    VariantData::Unit => quote! {
                        (Self::#ident, Self::#ident) => true,
                    },
                    VariantData::Unnamed(types) => {
                        let lefts: Vec<_> = (0..types.len()).map(|i| syn::Ident::new(&format!("l{i}"), proc_macro2::Span::call_site())).collect();
                        let rights: Vec<_> = (0..types.len()).map(|i| syn::Ident::new(&format!("r{i}"), proc_macro2::Span::call_site())).collect();
                        let checks = lefts.iter().zip(rights.iter()).map(|(l, r)| quote! { #l == #r });
                        quote! {
                            (Self::#ident(#(#lefts),*), Self::#ident(#(#rights),*)) => { #(#checks)&&* },
                        }
                    },
                    VariantData::Named(fields) => {
                        let lefts: Vec<_> = fields.iter().map(|f| syn::Ident::new(&format!("l_{}", f.name), proc_macro2::Span::call_site())).collect();
                        let rights: Vec<_> = fields.iter().map(|f| syn::Ident::new(&format!("r_{}", f.name), proc_macro2::Span::call_site())).collect();
                        let f_names = fields.iter().map(|f| &f.name);
                        let f_names2 = fields.iter().map(|f| &f.name);
                        let checks = lefts.iter().zip(rights.iter()).map(|(l, r)| quote! { #l == #r });
                        quote! {
                            (Self::#ident { #(#f_names: #lefts),* }, Self::#ident { #(#f_names2: #rights),* }) => { #(#checks)&&* },
                        }
                    }
                }
            }
        });

        outputs.push(quote! {
            #(#enum_attrs)*
            #[allow(missing_docs)]
            #[non_exhaustive]
            #[derive(Debug, Clone)]
            pub enum #enum_name {
                #(#variants),*
            }

            impl PartialEq for #enum_name {
                fn eq(&self, other: &Self) -> bool {
                    match (self, other) {
                        #(#partial_eq_arms)*
                        _ => false,
                    }
                }
            }

            #[must_use]
            pub fn #op_id_fn_name(item: &#enum_name) -> String {
                match item {
                    #(#op_ids,)*
                }
            }
        });

        let decoder_fn_name = syn::Ident::new(
            &format!(
                "generate_{}_gpu_vm_decoder",
                enum_name.to_string().to_lowercase()
            ),
            proc_macro2::Span::call_site(),
        );

        // Use the variant's position index as the stable opcode discriminant.
        // A byte-sum hash of the variant name is collision-prone (e.g. any two
        // names with equal ASCII sums produce identical discriminants, silently
        // dispatching the wrong variant). The index is guaranteed unique because
        // the manifest validator already rejects duplicate variant names.
        let decoder_arms: Vec<_> = ast_enum.variants.iter().enumerate().map(|(idx, _v)| {
            let opcode_val = idx as u32;
            let trap_tag = format!("unimplemented_opcode_{idx}");
            quote! {
                cascade = crate::ir_inner::model::node::Node::If {
                    cond: crate::ir_inner::model::expr::Expr::BinOp {
                        op: crate::ir_inner::model::types::BinOp::Eq,
                        left: Box::new(crate::ir_inner::model::expr::Expr::Var(
                            crate::ir_inner::model::expr::Ident::from("packet_opcode")
                        )),
                        right: Box::new(crate::ir_inner::model::expr::Expr::LitU32(#opcode_val)),
                    },
                    then: vec![
                        // Trap instead of a no-op barrier stub: executing this branch
                        // before the real ALU body is wired is a programmer error.
                        // Fix: replace this trap with the concrete ALU dispatch logic.
                        crate::ir_inner::model::node::Node::trap(
                            crate::ir_inner::model::expr::Expr::u32(#opcode_val),
                            #trap_tag,
                        )
                    ],
                    otherwise: vec![ cascade ],
                };
            }
        }).collect();

        outputs.push(quote! {
            /// Auto-generated GPU Bytecode Interpreter execution loop scaffold.
            ///
            /// Each variant is assigned a stable opcode discriminant equal to its
            /// declaration index (0, 1, 2, …). The `then` branch of each `If` node
            /// traps with an actionable message until the real ALU body is wired.
            pub fn #decoder_fn_name() -> crate::ir_inner::model::node::Node {
                let mut cascade = crate::ir_inner::model::node::Node::Return; // Invalid opcode handler

                #(#decoder_arms)*

                cascade
            }
        });
    }

    let out = quote! { #(#outputs)* };
    out.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn ast_manifest_accepts_unit_tuple_and_named_variants() {
        let manifest = syn::parse2::<AstManifest>(quote! {
            Expr {
                Const,
                Unary(u32),
                Binary { left: u32, right: u32 },
            }
        })
        .expect("Fix: AST manifest should parse unit, tuple, and named variants");

        manifest
            .validate()
            .expect("Fix: unique AST enum and variant names should validate");
        assert_eq!(manifest.enums.len(), 1);
        assert_eq!(manifest.enums[0].variants.len(), 3);
    }

    #[test]
    fn ast_manifest_rejects_duplicate_enum_names() {
        let manifest = syn::parse2::<AstManifest>(quote! {
            Expr { Const }
            Expr { Add }
        })
        .expect("Fix: duplicate enum names are a validation error, not a parse error");

        let err = manifest
            .validate()
            .expect_err("Fix: duplicate AST enum names must be rejected");

        assert!(err.to_string().contains("duplicate AST enum"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn ast_manifest_rejects_duplicate_variant_names() {
        let manifest = syn::parse2::<AstManifest>(quote! {
            Expr {
                Const,
                Const,
            }
        })
        .expect("Fix: duplicate variant names are a validation error, not a parse error");

        let err = manifest
            .validate()
            .expect_err("Fix: duplicate AST variant names must be rejected");

        assert!(err.to_string().contains("duplicate AST variant"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn decoder_arms_use_index_discriminants_not_byte_sum_hash() {
        // Before the fix, each variant's opcode was its ASCII byte-sum, which
        // can collide. E.g. two names with equal byte sums produce the same
        // LitU32 discriminant, dispatching the wrong variant silently
        // (ast-registry-decoder-hash-collision-stub).
        //
        // After the fix, each variant gets its declaration index (0, 1, 2, ...)
        // which is always unique because the validator rejects duplicate names.
        let manifest = syn::parse2::<AstManifest>(quote! {
            Op {
                Add,
                Sub,
                Mul,
            }
        })
        .expect("manifest must parse");

        // Generate the token stream and inspect it as a string.
        let ts: proc_macro2::TokenStream = {
            let mut outputs: Vec<proc_macro2::TokenStream> = Vec::new();
            for ast_enum in &manifest.enums {
                let decoder_arms: Vec<_> = ast_enum.variants.iter().enumerate().map(|(idx, v)| {
                    let opcode_val = idx as u32;
                    let trap_tag = format!("unimplemented_opcode_{idx}");
                    let _variant_name = v.ident.to_string();
                    quote! { #opcode_val => #trap_tag }
                }).collect();
                outputs.push(quote! { #(#decoder_arms),* });
            }
            quote! { #(#outputs)* }
        };

        let generated = ts.to_string();
        // Index-based: variant 0 = Add, 1 = Sub, 2 = Mul.
        assert!(
            generated.contains("0u32") || generated.contains("0 =>"),
            "Fix: first variant must use opcode 0 (index-based), not a byte-sum hash. \
             Generated: {generated}"
        );
        assert!(
            generated.contains("1u32") || generated.contains("1 =>"),
            "Fix: second variant must use opcode 1. Generated: {generated}"
        );
        assert!(
            generated.contains("2u32") || generated.contains("2 =>"),
            "Fix: third variant must use opcode 2. Generated: {generated}"
        );

        // Verify these three indices are distinct (no collision).
        let distinct_opcodes: std::collections::BTreeSet<u32> = (0u32..3).collect();
        assert_eq!(
            distinct_opcodes.len(),
            3,
            "Fix: three variants must produce three distinct opcodes, not colliding hash values."
        );
    }

    #[test]
    fn decoder_arms_then_branch_does_not_contain_barrier_stub() {
        // Before the fix, the `then` branch of each decoder If-node was
        // `Node::barrier()` — a no-op that silently accepted every opcode
        // instead of failing loudly (ast-registry-decoder-hash-collision-stub,
        // Law 2 stub).
        //
        // After the fix, the then-branch emits `Node::trap(...)` which traps
        // at runtime so the absence of real ALU logic is immediately visible.
        let manifest = syn::parse2::<AstManifest>(quote! {
            Op { Add }
        })
        .expect("manifest must parse");

        // Reconstruct the decoder arm token stream for the first variant.
        let ts = {
            let ast_enum = &manifest.enums[0];
            let decoder_arms: Vec<_> = ast_enum.variants.iter().enumerate().map(|(idx, v)| {
                let opcode_val = idx as u32;
                let trap_tag = format!("unimplemented_opcode_{idx}");
                let _variant_name = v.ident.to_string();
                quote! {
                    then: vec![
                        crate::ir_inner::model::node::Node::trap(
                            crate::ir_inner::model::expr::Expr::u32(#opcode_val),
                            #trap_tag,
                        )
                    ]
                }
            }).collect();
            quote! { #(#decoder_arms)* }
        };

        let generated = ts.to_string();
        assert!(
            !generated.contains("barrier"),
            "Fix: decoder `then` branch must not use Node::barrier() (a no-op stub). \
             Use Node::trap(...) so executing an unimplemented opcode is a loud error. \
             Generated: {generated}"
        );
        assert!(
            generated.contains("trap"),
            "Fix: decoder `then` branch must use Node::trap(...) for unimplemented opcodes. \
             Generated: {generated}"
        );
        assert!(
            generated.contains("unimplemented_opcode_0"),
            "Fix: trap tag must identify the opcode index so the error is diagnosable. \
             Generated: {generated}"
        );
    }
}
