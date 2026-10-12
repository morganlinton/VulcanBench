//! `define_op!` function-like macro  -  single-call op registration.
//!
//! Expands to: op type struct, associated `program()`, `LAWS` slice, and
//! an `inventory::submit!` registration that vyre's `DialectRegistry`
//! discovers at startup.
//!
//! Example:
//!
//! ```ignore
//! vyre_macros::define_op! {
//!     id = "primitive.bitwise.xor",
//!     dialect = "primitive.bitwise",
//!     category = A,
//!     inputs = ["u32", "u32"],
//!     outputs = ["u32"],
//!     laws = [Commutative, Associative, Identity { element: 0 }],
//!     program = |a, b| ::vyre::ir::Expr::BinOp {
//!         op: ::vyre::ir::BinOp::Xor,
//!         left: Box::new(a),
//!         right: Box::new(b),
//!     },
//! }
//! ```
//!
//! The macro body lives in this module; the public entry is
//! `crate::define_op` re-exported from lib.rs.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Expr, LitStr, Token};

struct DefineOpArgs {
    id: LitStr,
    dialect: LitStr,
    category: syn::Ident,
    inputs: Vec<LitStr>,
    outputs: Vec<LitStr>,
    laws: Vec<Expr>,
    program: Expr,
}

impl Parse for DefineOpArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut id: Option<LitStr> = None;
        let mut dialect: Option<LitStr> = None;
        let mut category: Option<syn::Ident> = None;
        let mut inputs: Vec<LitStr> = Vec::new();
        let mut outputs: Vec<LitStr> = Vec::new();
        let mut laws: Vec<Expr> = Vec::new();
        let mut program: Option<Expr> = None;
        let mut seen_keys = std::collections::BTreeSet::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            let key_name = crate::parse_helpers::reject_duplicate_key(&mut seen_keys, &key)?;
            input.parse::<Token![=]>()?;
            match key_name.as_str() {
                "id" => id = Some(input.parse()?),
                "dialect" => dialect = Some(input.parse()?),
                "category" => category = Some(input.parse()?),
                "inputs" => inputs = parse_str_array(input)?,
                "outputs" => outputs = parse_str_array(input)?,
                "laws" => laws = crate::parse_helpers::parse_expr_array(input)?,
                "program" => program = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown define_op! argument `{other}`. Fix: use id, dialect, category, inputs, outputs, laws, or program."),
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            id: id.ok_or_else(|| {
                input.error("missing `id = \"...\"`. Fix: declare a stable operation id.")
            })?,
            dialect: dialect.ok_or_else(|| {
                input.error("missing `dialect = \"...\"`. Fix: declare the owning dialect name.")
            })?,
            category: category.ok_or_else(|| {
                input.error("missing `category = A|B|C`. Fix: choose a dialect category variant.")
            })?,
            inputs,
            outputs,
            laws,
            program: program.ok_or_else(|| {
                input.error("missing `program = ...`. Fix: provide an expression that builds a vyre Program.")
            })?,
        })
    }
}

fn parse_str_array(input: ParseStream<'_>) -> syn::Result<Vec<LitStr>> {
    crate::parse_helpers::parse_litstr_array(
        input,
        "expected string literal. Fix: use string type names such as [\"u32\"].",
    )
}

pub(crate) fn define_op_impl(item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(item as DefineOpArgs);
    let id = args.id;
    let dialect = args.dialect;
    let category = args.category;
    let inputs = &args.inputs;
    let outputs = &args.outputs;
    let laws = &args.laws;
    let program = args.program;

    quote! {
        ::inventory::submit! {
            ::vyre::dialect::OpDefRegistration::new(|| ::vyre::dialect::OpDef {
                id: #id,
                dialect: #dialect,
                category: ::vyre::dialect::Category::#category,
                signature: ::vyre::dialect::Signature {
                    inputs: &[
                        #( ::vyre::dialect::TypedParam { name: "", ty: #inputs } ),*
                    ],
                    outputs: &[
                        #( ::vyre::dialect::TypedParam { name: "", ty: #outputs } ),*
                    ],
                    attrs: &[],
                    bytes_extraction: false,
                },
                lowerings: ::vyre::dialect::LoweringTable::empty(),
                laws: &[ #( ::vyre::ops::AlgebraicLaw::#laws ),* ],
                compose: {
                    fn __vyre_compose_program() -> ::vyre::ir::Program {
                        #program
                    }
                    Some(__vyre_compose_program)
                },
            })
        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn define_op_args_parse_complete_operation_contract() {
        let args = syn::parse2::<DefineOpArgs>(quote! {
            id = "primitive.bitwise.xor",
            dialect = "primitive.bitwise",
            category = A,
            inputs = ["u32", "u32"],
            outputs = ["u32"],
            laws = [Commutative, Associative, Identity { element: 0 }],
            program = || ::vyre::ir::Program::empty(),
        })
        .expect("Fix: complete define_op! declaration should parse");

        assert_eq!(args.id.value(), "primitive.bitwise.xor");
        assert_eq!(args.dialect.value(), "primitive.bitwise");
        assert_eq!(args.category.to_string(), "A");
        assert_eq!(
            args.inputs.iter().map(LitStr::value).collect::<Vec<_>>(),
            vec!["u32", "u32"]
        );
        assert_eq!(
            args.outputs.iter().map(LitStr::value).collect::<Vec<_>>(),
            vec!["u32"]
        );
        assert_eq!(args.laws.len(), 3);
    }

    #[test]
    fn define_op_args_reject_unknown_argument() {
        let err = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            dialect = "d",
            category = A,
            inputs = [],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
            owner = "consumer",
        })
        .err()
        .expect("Fix: define_op! must reject unknown arguments");

        assert!(err.to_string().contains("unknown define_op! argument"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn define_op_args_reject_duplicate_top_level_argument() {
        let err = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            id = "y",
            dialect = "d",
            category = A,
            inputs = [],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
        })
        .err()
        .expect("Fix: define_op! must reject duplicate top-level arguments");

        assert!(err.to_string().contains("duplicate macro argument `id`"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn define_op_args_reject_non_string_signature_entries() {
        let err = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            dialect = "d",
            category = A,
            inputs = [u32],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
        })
        .err()
        .expect("Fix: define_op! signatures must use string literal type names");

        assert!(err.to_string().contains("expected string literal"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn define_op_args_reject_missing_program() {
        let err = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            dialect = "d",
            category = A,
            inputs = [],
            outputs = [],
            laws = [],
        })
        .err()
        .expect("Fix: define_op! must require a program expression");

        assert!(err.to_string().contains("missing `program = ...`"));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn define_op_args_reject_each_missing_required_identity_field() {
        let missing_id = syn::parse2::<DefineOpArgs>(quote! {
            dialect = "d",
            category = A,
            inputs = [],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
        })
        .err()
        .expect("Fix: define_op! must require an id.");
        assert!(missing_id.to_string().contains("missing `id = \"...\"`"));
        assert!(missing_id.to_string().contains("Fix:"));

        let missing_dialect = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            category = A,
            inputs = [],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
        })
        .err()
        .expect("Fix: define_op! must require a dialect.");
        assert!(missing_dialect
            .to_string()
            .contains("missing `dialect = \"...\"`"));
        assert!(missing_dialect.to_string().contains("Fix:"));

        let missing_category = syn::parse2::<DefineOpArgs>(quote! {
            id = "x",
            dialect = "d",
            inputs = [],
            outputs = [],
            laws = [],
            program = || ::vyre::ir::Program::empty(),
        })
        .err()
        .expect("Fix: define_op! must require a category.");
        assert!(missing_category
            .to_string()
            .contains("missing `category = A|B|C`"));
        assert!(missing_category.to_string().contains("Fix:"));
    }
}
