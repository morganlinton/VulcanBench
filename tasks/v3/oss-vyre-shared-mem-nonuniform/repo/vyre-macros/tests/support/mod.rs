#![allow(unreachable_pub)]

pub mod ir {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Program {
        pub id: u64,
    }
}

pub mod optimizer {
    use super::ir::Program;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PassMetadata {
        pub name: &'static str,
        pub requires: &'static [&'static str],
        pub invalidates: &'static [&'static str],
        pub phase: PassPhase,
        pub boundary_class: PassBoundaryClass,
        pub requires_caps: &'static [&'static str],
        pub preserves_abi: bool,
        pub cost_model_family: CostModelFamily,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PassPhase {
        Unclassified,
        Canonicalization,
        ScalarAlgebra,
        Loop,
        Memory,
        FusionCse,
        Sync,
        Specialization,
        Cleanup,
        Dataflow,
        Megakernel,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PassBoundaryClass {
        Unknown,
        AbiPreserving,
        AbiChanging,
        BackendAware,
        RuntimeAware,
        DomainSpecific,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CostModelFamily {
        Unknown,
        Scalar,
        Loop,
        Memory,
        Fusion,
        Sync,
        Dataflow,
        Megakernel,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PassAnalysis {
        pub should_run: bool,
    }

    impl PassAnalysis {
        pub const RUN: Self = Self { should_run: true };
        pub const SKIP: Self = Self { should_run: false };
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PassResult {
        pub program: Program,
        pub changed: bool,
    }

    pub fn pass_result(program: Program, changed: bool) -> PassResult {
        PassResult { program, changed }
    }

    pub fn unchanged(program: Program) -> PassResult {
        pass_result(program, false)
    }

    pub mod private {
        pub trait Sealed {}
    }

    pub trait ProgramPass: private::Sealed + Send + Sync {
        fn metadata(&self) -> PassMetadata;
        fn analyze(&self, program: &Program) -> PassAnalysis;
        fn transform(&self, program: Program) -> PassResult;
        fn fingerprint(&self, program: &Program) -> u64;
    }

    pub struct ProgramPassRegistration {
        pub metadata: PassMetadata,
        pub factory: fn() -> Box<dyn ProgramPass>,
    }

    inventory::collect!(ProgramPassRegistration);

    pub fn fingerprint_program(program: &Program) -> u64 {
        program.id ^ 0x9e37_79b9_7f4a_7c15
    }
}

pub mod ops {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AlgebraicLaw {
        Commutative,
        Associative,
        Identity { element: u32 },
    }

    pub trait AlgebraicLawProvider {
        fn laws() -> &'static [AlgebraicLaw];
    }
}

pub mod dialect {
    use super::ir::Program;
    use super::ops::AlgebraicLaw;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Category {
        A,
        B,
        C,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TypedParam {
        pub name: &'static str,
        pub ty: &'static str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Signature {
        pub inputs: &'static [TypedParam],
        pub outputs: &'static [TypedParam],
        pub attrs: &'static [TypedParam],
        pub bytes_extraction: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct LoweringTable;

    impl LoweringTable {
        pub const fn empty() -> Self {
            Self
        }
    }

    pub struct OpDef {
        pub id: &'static str,
        pub dialect: &'static str,
        pub category: Category,
        pub signature: Signature,
        pub lowerings: LoweringTable,
        pub laws: &'static [AlgebraicLaw],
        pub compose: Option<fn() -> Program>,
    }

    pub struct OpDefRegistration {
        pub factory: fn() -> OpDef,
    }

    impl OpDefRegistration {
        pub const fn new(factory: fn() -> OpDef) -> Self {
            Self { factory }
        }
    }

    inventory::collect!(OpDefRegistration);
}

pub mod ir_inner {
    pub mod model {
        pub mod expr {
            #[derive(Clone, Debug, PartialEq, Eq)]
            pub struct Ident(pub String);

            impl From<&str> for Ident {
                fn from(value: &str) -> Self {
                    Self(value.to_owned())
                }
            }

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum Expr {
                Var(Ident),
                LitU32(u32),
                BinOp {
                    op: super::types::BinOp,
                    left: Box<Expr>,
                    right: Box<Expr>,
                },
            }

            impl Expr {
                /// Mirror of the real `vyre_foundation::ir::Expr::u32` constructor
                /// the `vyre_ast_registry!` decoder cascade emits.
                pub fn u32(value: u32) -> Self {
                    Self::LitU32(value)
                }
            }
        }

        pub mod types {
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum BinOp {
                Eq,
            }
        }

        pub mod node {
            #[derive(Clone, Debug, PartialEq, Eq)]
            pub enum Node {
                If {
                    cond: super::expr::Expr,
                    then: Vec<Node>,
                    otherwise: Vec<Node>,
                },
                Barrier,
                Return,
                Trap {
                    address: super::expr::Expr,
                    tag: super::expr::Ident,
                },
            }

            impl Node {
                pub fn barrier() -> Self {
                    Self::Barrier
                }

                /// Mirror of the real `vyre_foundation::ir::Node::trap(address, tag)`
                /// constructor the `vyre_ast_registry!` decoder cascade emits for
                /// each not-yet-wired ALU opcode branch.
                pub fn trap(address: super::expr::Expr, tag: impl Into<super::expr::Ident>) -> Self {
                    Self::Trap {
                        address,
                        tag: tag.into(),
                    }
                }
            }
        }
    }
}
