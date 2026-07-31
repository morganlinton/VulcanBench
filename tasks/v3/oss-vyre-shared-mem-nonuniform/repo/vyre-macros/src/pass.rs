use crate::parse_helpers;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Fields, ItemStruct, LitBool, LitStr, Token};

pub(crate) struct PassArgs {
    pub(crate) name: LitStr,
    pub(crate) requires: Vec<LitStr>,
    pub(crate) invalidates: Vec<LitStr>,
    pub(crate) phase: Option<LitStr>,
    pub(crate) boundary_class: Option<LitStr>,
    pub(crate) requires_caps: Vec<LitStr>,
    pub(crate) preserves_abi: Option<LitBool>,
    pub(crate) cost_model_family: Option<LitStr>,
    pub(crate) analyze_always: bool,
}

impl Parse for PassArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut name = None;
        let mut requires = Vec::new();
        let mut invalidates = Vec::new();
        let mut phase = None;
        let mut boundary_class = None;
        let mut requires_caps = Vec::new();
        let mut preserves_abi = None;
        let mut cost_model_family = None;
        let mut analyze_always = false;
        let mut seen_keys = std::collections::BTreeSet::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            let key_name = parse_helpers::reject_duplicate_key(&mut seen_keys, &key)?;
            input.parse::<Token![=]>()?;
            match key_name.as_str() {
                "name" => name = Some(input.parse()?),
                "requires" => {
                    requires = parse_helpers::parse_litstr_array(
                        input,
                        "pass metadata arrays accept only string literals. Fix: use [\"analysis_name\"].",
                    )?
                }
                "invalidates" => {
                    invalidates = parse_helpers::parse_litstr_array(
                        input,
                        "pass metadata arrays accept only string literals. Fix: use [\"analysis_name\"].",
                    )?
                }
                "phase" => phase = Some(input.parse()?),
                "boundary_class" => boundary_class = Some(input.parse()?),
                "requires_caps" => {
                    requires_caps = parse_helpers::parse_litstr_array(
                        input,
                        "pass metadata arrays accept only string literals. Fix: use [\"analysis_name\"].",
                    )?
                }
                "preserves_abi" => preserves_abi = Some(input.parse()?),
                "cost_model_family" => cost_model_family = Some(input.parse()?),
                "analyze" => {
                    let value: LitStr = input.parse()?;
                    if value.value() == "always" {
                        analyze_always = true;
                    } else {
                        return Err(syn::Error::new_spanned(
                            value,
                            "unsupported analyze mode. Fix: use analyze = \"always\" or omit it.",
                        ));
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "unsupported vyre_pass argument. Fix: use name, requires, invalidates, phase, boundary_class, requires_caps, preserves_abi, cost_model_family, or analyze.",
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        validate_unique_string_literals("requires", &requires)?;
        validate_unique_string_literals("invalidates", &invalidates)?;
        validate_unique_string_literals("requires_caps", &requires_caps)?;

        Ok(Self {
            name: name.ok_or_else(|| input.error("missing pass name. Fix: add name = \"...\"."))?,
            requires,
            invalidates,
            phase,
            boundary_class,
            requires_caps,
            preserves_abi,
            cost_model_family,
            analyze_always,
        })
    }
}

pub(crate) fn pass_phase_tokens(value: Option<&LitStr>) -> syn::Result<proc_macro2::TokenStream> {
    let variant = match value.map(LitStr::value).as_deref() {
        None | Some("unclassified") => quote! { Unclassified },
        Some("canonicalization") => quote! { Canonicalization },
        Some("scalar_algebra") => quote! { ScalarAlgebra },
        Some("loop") => quote! { Loop },
        Some("memory") => quote! { Memory },
        Some("fusion_cse") => quote! { FusionCse },
        Some("sync") => quote! { Sync },
        Some("specialization") => quote! { Specialization },
        Some("cleanup") => quote! { Cleanup },
        Some("dataflow") => quote! { Dataflow },
        Some("megakernel") => quote! { Megakernel },
        Some(_) => {
            let Some(value) = value else {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "unsupported pass phase. Fix: pass a string literal phase or omit the attribute.",
                ));
            };
            return Err(syn::Error::new_spanned(
                value,
                "unsupported pass phase. Fix: use unclassified, canonicalization, scalar_algebra, loop, memory, fusion_cse, sync, specialization, cleanup, dataflow, or megakernel.",
            ));
        }
    };
    Ok(quote! { ::vyre::optimizer::PassPhase::#variant })
}

pub(crate) fn boundary_class_tokens(
    value: Option<&LitStr>,
) -> syn::Result<proc_macro2::TokenStream> {
    let variant = match value.map(LitStr::value).as_deref() {
        None | Some("unknown") => quote! { Unknown },
        Some("abi_preserving") => quote! { AbiPreserving },
        Some("abi_changing") => quote! { AbiChanging },
        Some("backend_aware") => quote! { BackendAware },
        Some("runtime_aware") => quote! { RuntimeAware },
        Some("domain_specific") => quote! { DomainSpecific },
        Some(_) => {
            let Some(value) = value else {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "unsupported pass boundary_class. Fix: pass a string literal boundary_class or omit the attribute.",
                ));
            };
            return Err(syn::Error::new_spanned(
                value,
                "unsupported pass boundary_class. Fix: use unknown, abi_preserving, abi_changing, backend_aware, runtime_aware, or domain_specific.",
            ));
        }
    };
    Ok(quote! { ::vyre::optimizer::PassBoundaryClass::#variant })
}

pub(crate) fn cost_model_family_tokens(
    value: Option<&LitStr>,
) -> syn::Result<proc_macro2::TokenStream> {
    let variant = match value.map(LitStr::value).as_deref() {
        None | Some("unknown") => quote! { Unknown },
        Some("scalar") => quote! { Scalar },
        Some("loop") => quote! { Loop },
        Some("memory") => quote! { Memory },
        Some("fusion") => quote! { Fusion },
        Some("sync") => quote! { Sync },
        Some("dataflow") => quote! { Dataflow },
        Some("megakernel") => quote! { Megakernel },
        Some(_) => {
            let Some(value) = value else {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "unsupported pass cost_model_family. Fix: pass a string literal cost_model_family or omit the attribute.",
                ));
            };
            return Err(syn::Error::new_spanned(
                value,
                "unsupported pass cost_model_family. Fix: use unknown, scalar, loop, memory, fusion, sync, dataflow, or megakernel.",
            ));
        }
    };
    Ok(quote! { ::vyre::optimizer::CostModelFamily::#variant })
}

fn validate_unique_string_literals(field: &str, values: &[LitStr]) -> syn::Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        let text = value.value();
        if !seen.insert(text.clone()) {
            return Err(syn::Error::new_spanned(
                value,
                format!(
                    "duplicate vyre_pass {field} entry `{text}`. Fix: list each dependency, invalidation, or capability once."
                ),
            ));
        }
    }
    Ok(())
}

/// Register a unit struct as a `vyre::optimizer::ProgramPass`.
///
/// Expands to (a) a full `ProgramPass` trait impl that forwards to your inherent
/// `analyze` / `transform` methods plus the canonical optimizer
/// fingerprint and (b) an
/// `inventory::submit!` that adds the pass to the global registry so
/// `vyre::optimize()` picks it up automatically.
///
/// # Arguments
///
/// | Argument       | Type        | Meaning                                                             |
/// |----------------|-------------|---------------------------------------------------------------------|
/// | `name`         | string lit  | Stable pass name used in diagnostics / ordering.                    |
/// | `requires`     | `[&str]`    | Pass names that must fire before this one.                          |
/// | `invalidates`  | `[&str]`    | Analyses invalidated when this pass rewrites the program.           |
/// | `phase`        | string lit  | Optional scheduler phase.                                           |
/// | `boundary_class` | string lit | Optional architectural boundary class.                              |
/// | `requires_caps` | `[&str]`   | Optional backend/runtime capabilities required by the pass.          |
/// | `preserves_abi` | bool       | Whether public buffer ABI is preserved. Defaults to true.            |
/// | `cost_model_family` | string lit | Optional cost attribution family.                                |
///
/// # Required inherent methods on the annotated type
///
/// ```ignore
/// fn analyze_impl(program: &Program) -> PassAnalysis;
/// fn transform(program: Program) -> PassResult;
/// ```
///
/// # Example
///
/// ```ignore
/// use vyre::optimizer::{vyre_pass, PassAnalysis, PassResult};
/// use vyre::ir::Program;
///
/// #[vyre_pass(name = "fold_zero_add", requires = [], invalidates = [])]
/// pub struct FoldZeroAdd;
///
/// impl FoldZeroAdd {
///     fn analyze(_program: &Program) -> PassAnalysis { PassAnalysis::RUN }
///     fn transform(program: Program) -> PassResult {
///         // ... real rewrite ...
///         PassResult::from_programs(&program.clone(), program)
///     }
/// }
/// ```
///
/// After expansion, `vyre::optimize(p)` will pick up `FoldZeroAdd` through
/// the `inventory::collect!(ProgramPassRegistration)` entry emitted by the macro.
/// No manual registration needed.
pub(crate) fn vyre_pass_impl(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as PassArgs);
    let item = parse_macro_input!(item as ItemStruct);
    if !matches!(item.fields, Fields::Unit) {
        return syn::Error::new_spanned(
            &item.ident,
            "#[vyre_pass] supports only unit structs. Fix: move pass state into explicit scheduler/context storage and declare the pass as `pub struct PassName;`.",
        )
        .to_compile_error()
        .into();
    }
    let ident = &item.ident;
    let name = args.name;
    let requires = args.requires;
    let invalidates = args.invalidates;
    let requires_caps = args.requires_caps;
    let phase = match pass_phase_tokens(args.phase.as_ref()) {
        Ok(tokens) => tokens,
        Err(error) => return error.to_compile_error().into(),
    };
    let boundary_class = match boundary_class_tokens(args.boundary_class.as_ref()) {
        Ok(tokens) => tokens,
        Err(error) => return error.to_compile_error().into(),
    };
    let cost_model_family = match cost_model_family_tokens(args.cost_model_family.as_ref()) {
        Ok(tokens) => tokens,
        Err(error) => return error.to_compile_error().into(),
    };
    let preserves_abi = args.preserves_abi.map(|value| value.value).unwrap_or(true);
    let analyze_body = if args.analyze_always {
        quote! { ::vyre::optimizer::PassAnalysis::RUN }
    } else {
        quote! { Self::analyze_impl(program) }
    };
    let metadata = quote! {
        ::vyre::optimizer::PassMetadata {
            name: #name,
            requires: &[#(#requires),*],
            invalidates: &[#(#invalidates),*],
            phase: #phase,
            boundary_class: #boundary_class,
            requires_caps: &[#(#requires_caps),*],
            preserves_abi: #preserves_abi,
            cost_model_family: #cost_model_family,
        }
    };

    quote! {
        #item

        impl ::vyre::optimizer::private::Sealed for #ident {}

        impl ::vyre::optimizer::ProgramPass for #ident {
            #[inline]
            fn metadata(&self) -> ::vyre::optimizer::PassMetadata {
                #metadata
            }

            #[inline]
            fn analyze(&self, program: &::vyre::ir::Program) -> ::vyre::optimizer::PassAnalysis {
                #analyze_body
            }

            #[inline]
            fn transform(
                &self,
                program: ::vyre::ir::Program,
            ) -> ::vyre::optimizer::PassResult {
                Self::transform(program)
            }

            #[inline]
            fn fingerprint(&self, program: &::vyre::ir::Program) -> u64 {
                ::vyre::optimizer::fingerprint_program(program)
            }
        }

        ::inventory::submit! {
            ::vyre::optimizer::ProgramPassRegistration {
                metadata: #metadata,
                factory: || ::std::boxed::Box::new(#ident),
            }
        }
    }
    .into()
}
