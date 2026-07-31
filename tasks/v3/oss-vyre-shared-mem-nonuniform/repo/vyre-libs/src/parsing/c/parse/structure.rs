use crate::parsing::c::lex::tokens::*;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

mod body_scan;
mod call_graph;
mod caller_lookup;
mod calls;
mod functions;
mod harness;
mod predicates;
mod threaded;

pub use call_graph::c11_build_call_graph;
pub use calls::c11_extract_calls;
pub use functions::c11_extract_functions;

use body_scan::emit_body_open_scan;
use caller_lookup::emit_enclosing_function_lookup;
use predicates::*;
use threaded::*;
