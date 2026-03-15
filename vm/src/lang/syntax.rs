use hex;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

enum SymbolEntry {
    Slot(u8, bool),
    Bind(Box<dyn IRNode>),
    Const(Box<dyn IRNode>),
}

#[derive(Default)]
struct SyntaxMode {
    check_op: bool,
    expect_retval: bool,
    loop_depth: usize,
    is_ircode: bool,
}

#[derive(Default)]
struct SyntaxEmit {
    irnode: IRNodeArray,
    source_map: SourceMap,
}

#[derive(Default)]
struct SyntaxInjected {
    ext_params: Option<Vec<(String, ValueTy)>>,
    ext_libs: Option<Vec<(String, u8, Option<FieldAddress>)>>,
    ext_consts: Option<Vec<(String, Box<dyn IRNode>)>>,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Syntax {
    tokens: Vec<Token>,
    idx: usize,
    symbols: HashMap<String, SymbolEntry>,
    slot_used: HashSet<u8>,
    bdlibs: HashMap<String, (u8, Option<FieldAddress>)>,
    local_alloc: u8,
    mode: SyntaxMode,
    emit: SyntaxEmit,
    injected: SyntaxInjected,
}

include!("syntax_core.rs");
include!("syntax_call.rs");
include!("syntax_literal.rs");
include!("syntax_symbol.rs");
include!("syntax_expr.rs");
include!("syntax_stmt.rs");
include!("syntax_program.rs");
