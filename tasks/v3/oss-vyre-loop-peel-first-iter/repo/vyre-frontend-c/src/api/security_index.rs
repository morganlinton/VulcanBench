use std::path::Path;

use super::{
    lex_index::{decode_object_lex_index_from_container, CObjectLexIndex, CObjectToken},
    object_decode::{
        decode_object_abi_layout_from_container, decode_object_ast_from_container,
        decode_object_sema_scope_from_container, decode_object_semantic_graph_from_container,
    },
    structure_index::{
        decode_object_structure_index_from_container, CObjectCallRecord, CObjectFunctionRecord,
        CObjectStructureIndex,
    },
    CObjectAbiLayout, CObjectAst, CObjectSemaScope, CObjectSemanticGraph, CObjectSymbolRef,
};
use crate::object_format::parse_embedded_vyrecob2;

mod calls;
mod common;
mod decode;
mod model;
mod requirements;
mod stats;
mod symbols;
mod tokens;
mod validation;

pub use decode::{decode_object_security_index, decode_object_security_index_file};
pub use model::{
    CObjectFunctionCall, CObjectSecurityIndex, CObjectSecurityStats, CObjectSymbolToken,
};

use common::count_u64;
use validation::validate_security_cross_sections;
