use sys::Ret;
use crate::rt::{Token, verify_bytecodes, SourceMap, Bytecode};
use crate::ir::{convert_ir_to_bytecode, drop_irblock_wrap, IRNodeArray, IRNodeLeaf};
use crate::value::ValueTy;
use crate::lang::Syntax;
use crate::IRNode;
use crate::ir::{push_num, push_addr, push_bytes};
use field::Address;

/// Compiled code result - either IR codes or bytecodes
pub enum CompiledCode {
    IrCode(Vec<u8>),
    Bytecode(Vec<u8>),
}

/// Parse a constant value string into an IRNode
/// Format: "type:value" where type is uint, bool, bytes, address, or string
fn parse_const_value(value_str: &str) -> Ret<Box<dyn IRNode>> {
    if value_str.starts_with("uint:") {
        let num_str = &value_str[5..];
        let num: u128 = num_str.parse().map_err(|_| format!("invalid uint constant: {}", value_str))?;
        return Ok(push_num(num));
    }
    if value_str.starts_with("bool:") {
        let bool_str = &value_str[5..];
        return Ok(match bool_str {
            "true" => Box::new(IRNodeLeaf {
                hrtv: true,
                inst: Bytecode::PTRUE,
                text: "true".to_string(),
            }),
            "false" => Box::new(IRNodeLeaf {
                hrtv: true,
                inst: Bytecode::PFALSE,
                text: "false".to_string(),
            }),
            _ => return Err(format!("invalid bool constant: {}", value_str).into()),
        });
    }
    if value_str.starts_with("bytes:0x") {
        let hex_str = &value_str[10..]; // skip "bytes:0x"
        let bytes = hex::decode(hex_str).map_err(|_| format!("invalid hex bytes constant: {}", value_str))?;
        return push_bytes(&bytes);
    }
    if value_str.starts_with("address:") {
        let addr_str = &value_str[8..];
        let addr = field::Address::from_readable(addr_str)
            .map_err(|_| format!("invalid address constant: {}", value_str))?;
        return Ok(push_addr(addr));
    }
    if value_str.starts_with("string:") {
        let str_content = &value_str[7..];
        let bytes = str_content.as_bytes().to_vec();
        return push_bytes(&bytes);
    }
    // Try parsing as plain uint
    if let Ok(num) = value_str.parse::<u128>() {
        return Ok(push_num(num));
    }
    Err(format!("unrecognized constant format: {}", value_str).into())
}

/// Compile function/abstract body tokens to IR or bytecode
///
/// # Arguments
/// * `body_tokens` - The tokens of the function body
/// * `args` - Parameter names and types
/// * `libs` - Contract-level libraries (name, address)
/// * `consts` - Contract-level constants (name, value_string)
/// * `is_ircode` - Whether to compile to IR code or bytecode
///
/// # Returns
/// * `Ok((IRNodeArray, CompiledCode, SourceMap))` - The IR nodes, compiled code, and source map
pub fn compile_body(
    body_tokens: Vec<Token>,
    args: Vec<(String, ValueTy)>,
    libs: &[(String, Address)],
    consts: &[(String, String)],
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

    // Inject contract-level constants
    if !consts.is_empty() {
        let mut const_nodes = Vec::new();
        for (name, value_str) in consts {
            let node = parse_const_value(value_str)?;
            const_nodes.push((name.clone(), node));
        }
        syntax = syntax.with_consts(const_nodes);
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
