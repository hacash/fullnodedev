
use crate::native::{NativeFunc, NativeEnv};

/*
    Verify bytecode validity and return the instruction table.
*/
pub fn convert_and_check(cap: &SpaceCap, ctype: CodeType, codes: &[u8], height: u64) -> VmrtRes<Vec<u8>> {
    use CodeType::*;
    let bytecodes = match ctype {
        IRNode =>  &runtime_irs_to_bytecodes(codes, height)?,
        Bytecode => codes
    };
    // check size
    if bytecodes.len() > cap.one_function_size {
        return itr_err_code!(CodeTooLong)
    }
    // verify inst
    verify_bytecodes_with_limits(bytecodes, cap.max_value_size)
}


pub fn verify_bytecodes(codes: &[u8]) -> VmrtRes<Vec<u8>> {
    verify_bytecodes_with_limits(codes, SpaceCap::new(1).max_value_size)
}

fn verify_bytecodes_with_limits(codes: &[u8], max_push_buf_len: usize) -> VmrtRes<Vec<u8>> {
    // check empty
    let cl = codes.len();
    if cl <= 0 {
        return itr_err_code!(CodeEmpty)
    }
    if cl > u16::MAX as usize {
        return itr_err_code!(CodeTooLong)
    }
    // check inst valid
    let (instable, jumpdests) = verify_valid_instruction(codes, max_push_buf_len)?;
    // check jump dests
    verify_jump_dests(&instable, &jumpdests)?;
    // ok finish
    Ok(instable)
}


/// Ensure the last instruction is a terminal one (RET/END/ERR/ABT or call).
/// Failure (CodeNotWithEnd) is a fitsh code compile error and propagates to the compiler
/// via compile_body -> parse_function -> parse_top_level -> fitshc::compile.
fn ensure_terminal_instruction(inst: Bytecode) -> VmrtErr {
    if let RET | END | ERR | ABT |
        CALLCODE | CALLPURE | CALLVIEW | CALLTHIS | CALLSELF | CALLSUPER | CALL // CALLDYN
    = inst {
        return Ok(())
    };
    // error
    itr_err_code!(CodeNotWithEnd)
}


/*

*/   
fn verify_valid_instruction(codes: &[u8], max_push_buf_len: usize) -> VmrtRes<(Vec<u8>, Vec<isize>)> {
    // use Bytecode::*;
    let cdlen = codes.len(); // end/ret/err/abt in tail
    let mut instable = vec![0u8; cdlen];
    let mut jumpdest = vec![];
    let mut i = 0;
    let mut cur = Bytecode::default();
    while i < cdlen {
        let curbt = codes[i];
        let inst: Bytecode = std_mem_transmute!(curbt);
        let meta = inst.metadata();
        if ! meta.valid {
            return itr_err_fmt!(InstInvalid, "{}", inst as u8)
        }
        match inst {
            IRBYTECODE | IRLIST | IRBLOCK | IRBLOCKR | IRIF | IRIFR | IRWHILE |
            IRBREAK | IRCONTINUE => {
                return itr_err_fmt!(InstInvalid, "IR bytecode {:?} is not allowed", inst)
            }
            _ => {}
        }
        cur = inst;
        instable[i] = 1; // yes is valid instruction
        i += 1;
        macro_rules! pu8 { () => {{
            if i >= cdlen { return itr_err_code!(InstParamsErr) }
            codes[i as usize]
        }}}
        macro_rules! pu16 { () => {{
            let r = i + 2;
            if r > cdlen { return itr_err_code!(InstParamsErr) }
            u16::from_be_bytes(codes[i as usize..r as usize].try_into().unwrap())
        }}}
        macro_rules! pi8 { () => {
            pu8!() as i8
        }}
        macro_rules! pi16 { () => {
            pu16!() as i16
        }}
        macro_rules! adddest { ($jt:expr) => {{
            jumpdest.push($jt)
        }}}
        match inst {
            // push buf
            PBUF  => {
                let l = pu8!() as usize;
                if l > max_push_buf_len {
                    return itr_err_fmt!(InstParamsErr, "PBUF size {} too large", l)
                }
                i += l
            }
            PBUFL => {
                let l = pu16!() as usize;
                if l > max_push_buf_len {
                    return itr_err_fmt!(InstParamsErr, "PBUFL size {} too large", l)
                }
                i += l
            }
            // ext/native
            EXTACTION | EXTENV | EXTVIEW => ensure_extend_call_id(inst, pu8!())?,
            NTFUNC => {
                let idx = pu8!();
                if !NativeFunc::has_idx(idx) {
                    return itr_err_fmt!(NativeFuncError, "native func idx {} not found", idx)
                }
            }
            NTENV => {
                let idx = pu8!();
                if !NativeEnv::has_idx(idx) {
                    return itr_err_fmt!(NativeEnvError, "native env idx {} not found", idx)
                }
            }
            // jump record
            JMPL  | BRL  => adddest!(pu16!() as isize),
            JMPS  | BRS  => adddest!(i as isize + pi8!() as isize + 1),
            JMPSL | BRSL | BRSLN => adddest!(i as isize + pi16!() as isize + 2),
            _ => {}
        };
        i += meta.param as usize;
        if i > cdlen {            
            return itr_err_code!(InstParamsErr)
        }        
        // CALLCODE must be immediately followed by END unless it is the final instruction.
        if let CALLCODE = inst {
            if i < cdlen {
                let nxt: Bytecode = std_mem_transmute!(codes[i]);
                if nxt != END {
                    return itr_err_fmt!(InstParamsErr, "CALLCODE must follow END")
                }
            }
        }
        // next
    }
    ensure_terminal_instruction(cur)?; // check end
    // finish orr
    Ok((instable, jumpdest))
}


//
fn verify_jump_dests(instable: &[u8], jumpdests: &[isize]) -> VmrtErr {
    let itlen = instable.len();
    let right = itlen as isize - 1;
    for jp in jumpdests {
        let j = *jp;
        if j < 0 || j > right {
            return itr_err_code!(JumpOverflow)
        }
        if 0 == instable[j as usize] {
            return itr_err_code!(JumpInDataSeg)
        }
    }
    // finish
    Ok(())
}
