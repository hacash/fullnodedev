use std::any::*;
use std::collections::*;
use std::iter;

use sys::*;

use super::ir::*;
use super::rt::*;
use super::value::*;
use super::*;
use super::rt::Token::*;
// use super::rt::TokenType::*;

use super::native::*;

include! {"print_option.rs"}
include! {"decompilation_helper.rs"}

pub enum ArgvMode {
    Concat,
    PackList,
}

include! {"interface.rs"}
include! {"tokenizer.rs"}
include! {"funcs.rs"}
include! {"syntax.rs"}
include! {"formater.rs"}
include! {"test.rs"}

pub fn lang_to_irnode_with_sourcemap(langscript: &str) -> Ret<(IRNodeArray, SourceMap)> {
    let tkr = Tokenizer::new(langscript.as_bytes());
    let tks = tkr.parse()?;
    let syx = Syntax::new(tks);
    syx.parse()
}

pub fn lang_to_irnode(langscript: &str) -> Ret<IRNodeArray> {
    let (block, _) = lang_to_irnode_with_sourcemap(langscript)?;
    Ok(block)
}

pub fn lang_to_ircode(langscript: &str) -> Ret<Vec<u8>> {
    let ir = lang_to_irnode(langscript)?;
    Ok(ir.serialize().split_off(3)) // drop block op and length bytes
}

pub fn lang_to_ircode_with_sourcemap(langscript: &str) -> Ret<(Vec<u8>, SourceMap)> {
    let (ir, smap) = lang_to_irnode_with_sourcemap(langscript)?;
    Ok((ir.serialize().split_off(3), smap)) // drop block op and length bytes
}

pub fn irnode_to_lang_with_sourcemap(block: IRNodeArray, smap: &SourceMap) -> Ret<String> {
    let opt = PrintOption::new("  ", 0)
        .with_source_map(smap)
        .with_trim_root_block(true)
        .with_trim_head_alloc(true)
        .with_trim_param_unpack(true);
    Ok(Formater::new(&opt).print(&block))
}

pub fn irnode_to_lang(block: IRNodeArray) -> Ret<String> {
    let opt = PrintOption::new("  ", 0)
        .with_trim_root_block(true)
        .with_trim_head_alloc(true)
        .with_trim_param_unpack(true);
    Ok(Formater::new(&opt).print(&block))
}

fn format_ircode_to_lang(ircode: &Vec<u8>, map: Option<&SourceMap>) -> VmrtRes<String> {
    let mut seek = 0;
    let block = parse_ir_block(ircode, &mut seek)?;
    let mut opt = PrintOption::new("  ", 0)
        .with_trim_root_block(true)
        .with_trim_head_alloc(true)
        .with_trim_param_unpack(true);
    if let Some(map) = map {
        opt = opt.with_source_map(map);
    }
    Ok(Formater::new(&opt).print(&block))
}

pub fn ircode_to_lang_with_sourcemap(ircode: &Vec<u8>, smap: &SourceMap) -> Ret<String> {
    format_ircode_to_lang(ircode, Some(smap)).map_err(|e| e.to_string())
}

pub fn ircode_to_lang(ircode: &Vec<u8>) -> Ret<String> {
    format_ircode_to_lang(ircode, None).map_err(|e| e.to_string())
}

pub fn lang_to_bytecode(langscript: &str) -> Ret<Vec<u8>> {
    let ir = lang_to_irnode(langscript)?;
    let codes = ir.codegen()?;
    Ok(codes)
}
