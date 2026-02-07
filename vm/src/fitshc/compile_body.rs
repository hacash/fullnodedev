use sys::Ret;
use crate::rt::{Token, verify_bytecodes, SourceMap};
use crate::ir::{convert_ir_to_bytecode, drop_irblock_wrap, IRNodeArray};
use crate::value::ValueTy;
use crate::lang::Syntax;
use crate::IRNode;
use field::Address;

/// Compiled code result - either IR codes or bytecodes
pub enum CompiledCode {
    IrCode(Vec<u8>),
    Bytecode(Vec<u8>),
}

/// Compile function/abstract body tokens to IR or bytecode
/// 
/// # Arguments
/// * `body_tokens` - The tokens of the function body
/// * `args` - Parameter names and types
/// * `libs` - Contract-level libraries (name, address)
/// * `is_ircode` - Whether to compile to IR code or bytecode
/// 
/// # Returns
/// * `Ok((IRNodeArray, CompiledCode, SourceMap))` - The IR nodes, compiled code, and source map
pub fn compile_body(
    body_tokens: Vec<Token>,
    args: Vec<(String, ValueTy)>,
    libs: &[(String, Address)],
    is_ircode: bool,
) -> Ret<(IRNodeArray, CompiledCode, SourceMap)> {
    // Setup syntax parser
    let mut syntax = Syntax::new(body_tokens);
    
    // Inject contract-level libs (0-based order)
    if !libs.is_empty() {
        let lib_entries: Vec<_> = libs.iter().enumerate().map(|(idx, (name, addr))| {
            (name.clone(), idx as u8, Some(addr.clone()))
        }).collect();
        syntax = syntax.with_libs(lib_entries);
    }
    
    // Inject params and set compilation mode
    syntax = syntax.with_params(args).with_ircode(is_ircode);
    
    // Parse to IR nodes
    let (irnodes, source_map) = syntax.parse()?;
    
    // Generate code based on mode
    let compiled = if is_ircode {
        // IR mode: store raw block content (without IRBLOCK/IRBLOCKR wrapper)
        let ircodes = drop_irblock_wrap(irnodes.serialize())?;
        
        // Verify by converting to bytecode. Failures (e.g. CodeNotWithEnd, JumpOverflow) are
        // fitsh compile errors; they propagate via parse_function -> parse_top_level -> compile.
        let codes = convert_ir_to_bytecode(&ircodes).map_err(|e| e.to_string())?;
        verify_bytecodes(&codes).map_err(|e| e.to_string())?;
        
        CompiledCode::IrCode(ircodes)
    } else {
        // Bytecode mode: direct codegen; verify failures propagate as compile errors
        let bts = irnodes.codegen().map_err(|e| e.to_string())?;
        verify_bytecodes(&bts).map_err(|e| e.to_string())?;
        
        CompiledCode::Bytecode(bts)
    };
    
    Ok((irnodes, compiled, source_map))
}
