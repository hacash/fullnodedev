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
    List,
}

include! {"interface.rs"}
include! {"tokenizer.rs"}
include! {"funcs.rs"}
include! {"syntax.rs"}
include! {"formater.rs"}
include! {"test.rs"}

pub fn lang_to_irnode_with_sourcemap(langscript: &str) -> Ret<(IRNodeArray, SourceMap)> {
    let tkr = Tokenizer::new(langscript.as_bytes());
    let mut tks = tkr.parse()?;
    // The formatter may emit a file-level `{ ... }` wrapper when `trim_root_block` is disabled.
    // That wrapper is intended as a presentation detail, not a semantic block expression.
    // To keep decompile->recompile closed, treat an outermost brace pair that encloses the
    // entire file as a no-op wrapper and parse the inner content as the program body.
    if tks.len() >= 2 {
        if let (Partition('{'), Partition('}')) = (&tks[0], &tks[tks.len() - 1]) {
            let mut depth: isize = 0;
            let mut close_at: Option<usize> = None;
            for (i, tk) in tks.iter().enumerate() {
                match tk {
                    Partition('{') => depth += 1,
                    Partition('}') => {
                        depth -= 1;
                        if depth == 0 {
                            close_at = Some(i);
                            break;
                        }
                        if depth < 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if close_at == Some(tks.len() - 1) {
                tks.remove(0);
                tks.pop();
            }
        }
    }
    let syx = Syntax::new(tks).with_ircode(true);
    syx.parse()
}

pub fn lang_to_irnode(langscript: &str) -> Ret<IRNodeArray> {
    let (block, _) = lang_to_irnode_with_sourcemap(langscript)?;
    Ok(block)
}

pub fn lang_to_ircode(langscript: &str) -> Ret<Vec<u8>> {
    let ir = lang_to_irnode(langscript)?;
    drop_irblock_wrap(ir.serialize())
}

pub fn lang_to_ircode_with_sourcemap(langscript: &str) -> Ret<(Vec<u8>, SourceMap)> {
    let (ir, smap) = lang_to_irnode_with_sourcemap(langscript)?;
    Ok((drop_irblock_wrap(ir.serialize())?, smap))
}

pub fn irnode_to_lang_with_sourcemap(block: IRNodeArray, smap: &SourceMap) -> Ret<String> {
    let mut opt = PrintOption::new("  ", 0);
    opt.map = Some(smap);
    Ok(Formater::new(&opt).print(&block))
}

pub fn irnode_to_lang(block: IRNodeArray) -> Ret<String> {
    let opt = PrintOption::new("  ", 0);
    Ok(Formater::new(&opt).print(&block))
}

pub fn format_ircode_to_lang(ircode: &Vec<u8>, map: Option<&SourceMap>) -> VmrtRes<String> {
    let mut seek = 0;
    let block = parse_ir_block(ircode, &mut seek)?;
    let mut opt = PrintOption::new("  ", 0);
    if let Some(map) = map {
        opt.map = Some(map);
    }
    opt.trim_root_block = true;
    opt.trim_head_alloc = true;
    opt.trim_param_unpack = true;
    opt.hide_default_call_argv = true;
    opt.call_short_syntax = true;
    opt.flatten_call_list = true;
    opt.flatten_array_list = true;
    opt.flatten_syscall_cat = true;
    opt.recover_literals = true;
    Ok(Formater::new(&opt).print(&block))
}

pub fn ircode_to_lang_with_sourcemap(ircode: &Vec<u8>, smap: &SourceMap) -> Ret<String> {
    let mut seek = 0;
    let block = parse_ir_block(ircode, &mut seek)?;
    irnode_to_lang_with_sourcemap(block, smap).map_err(|e| e.to_string())
}

pub fn ircode_to_lang(ircode: &Vec<u8>) -> Ret<String> {
    let mut seek = 0;
    let block = parse_ir_block(ircode, &mut seek)?;
    irnode_to_lang(block).map_err(|e| e.to_string())
}

pub fn lang_to_bytecode(langscript: &str) -> Ret<Vec<u8>> {
    let ir = lang_to_irnode(langscript)?;
    let codes = ir.codegen()?;
    Ok(codes)
}
