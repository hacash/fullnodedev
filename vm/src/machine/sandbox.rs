const SANDBOX_TX_FEE_238: u64 = 100_000;
const SANDBOX_FUND_238: u64 = 1_000_000_000;

#[derive(Debug, Clone)]
pub struct SandboxSpec {
    pub contract: ContractAddress,
    pub function: String,
    pub args: Vec<Value>,
    pub caller: Option<Address>,
    pub gas_budget: Option<i64>,
    pub gas_max_byte: Option<u8>,
}

impl SandboxSpec {
    pub fn new(contract: ContractAddress, function: impl Into<String>) -> Self {
        Self {
            contract,
            function: function.into(),
            args: vec![],
            caller: None,
            gas_budget: None,
            gas_max_byte: None,
        }
    }

    pub fn args(mut self, args: Vec<Value>) -> Self {
        self.args = args;
        self
    }

    pub fn caller(mut self, caller: Address) -> Self {
        self.caller = Some(caller);
        self
    }

    pub fn gas_budget(mut self, gas_budget: i64) -> Self {
        self.gas_budget = Some(gas_budget);
        self
    }

    pub fn gas_max_byte(mut self, gas_max_byte: u8) -> Self {
        self.gas_max_byte = Some(gas_max_byte);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxResult {
    pub use_gas: i64,
    pub ret_val: Value,
}

pub fn sandbox_call(ctx: &mut dyn Context, spec: SandboxSpec) -> Ret<SandboxResult> {
    use rt::verify_bytecodes;

    let mut env = ctx.env().clone();
    let state = ctx.state().clone_state();
    let caller = spec.caller.unwrap_or_else(|| ctx.tx().main());
    let gas_extra = GasExtra::new(env.block.height);
    let (tx_gas_max, gas_budget) = match spec.gas_max_byte {
        Some(0) => return errf!("sandbox gas_max byte invalid: 0"),
        Some(gmx) => (gmx, decode_gas_budget(gmx)),
        None => {
            let budget_hint = spec
                .gas_budget
                .unwrap_or(gas_extra.max_gas_of_tx);
            let tx_gas_max = encode_gas_budget(budget_hint.max(1));
            let gas_budget = match spec.gas_budget {
                Some(v) if v > 0 => v.min(gas_extra.max_gas_of_tx),
                Some(v) => return errf!("sandbox gas budget invalid: {}", v),
                None => gas_extra.max_gas_of_tx,
            };
            (tx_gas_max, gas_budget)
        }
    };
    let mut tx = TransactionType3::new_by(caller, Amount::unit238(SANDBOX_TX_FEE_238), env.block.height);
    tx.addrlist = AddrOrList::from_list(vec![caller, spec.contract.into_addr()])?;
    tx.gas_max = Uint1::from(tx_gas_max);
    env.tx = create_tx_info(&tx);
    let mut temp_ctx = protocol::context::ContextInst::new(
        env,
        state,
        Box::new(protocol::state::EmptyLogs {}),
        &tx,
    );
    let hei = temp_ctx.env().block.height;
    let codes = build_call_codes(&spec.function, &spec.args)?;
    verify_bytecodes(&codes)?;
    let caller = temp_ctx.tx().main();
    protocol::operate::hac_add(
        &mut temp_ctx,
        &caller,
        &Amount::unit238(SANDBOX_FUND_238),
    )?;
    temp_ctx.gas_init_tx(gas_budget, gas_extra.gas_rate)?;
    let gas_before = Context::gas_remaining(&temp_ctx);
    let mut vmb = global_machine_manager().assign(hei);
    let ret_val = vmb.sandbox_main_call_raw(&mut temp_ctx, CodeType::Bytecode, codes.into())?;
    Ok(SandboxResult {
        use_gas: gas_before - Context::gas_remaining(&temp_ctx),
        ret_val,
    })
}

pub fn parse_sandbox_params(pms: &str) -> Ret<Vec<Value>> {
    let mut values = vec![];
    for part in pms.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (v, t) = match part.split_once(':') {
            Some((v, t)) => (v.trim(), t.trim()),
            None => (part, "nil"),
        };
        values.push(parse_one_param(t, v)?);
    }
    Ok(values)
}

pub fn build_call_codes(funcname: &str, args: &[Value]) -> Ret<Vec<u8>> {
    use rt::Bytecode::*;

    let mut codes = vec![];
    for arg in args {
        append_push_value_code(&mut codes, arg)?;
    }
    match crate::value::classify_call_args_len(args.len()).map_err(|e| e.to_string())? {
        crate::value::CallArgsPack::Nil => codes.push(PNIL as u8),
        crate::value::CallArgsPack::Raw => {}
        crate::value::CallArgsPack::Tuple => {
            codes.push(PU8 as u8);
            codes.push(args.len() as u8);
            codes.push(PACKTUPLE as u8);
        }
    }
    let fnsg = calc_func_sign(funcname);
    codes.push(CALLEXT as u8);
    codes.push(1);
    codes.extend_from_slice(&fnsg);
    codes.push(RET as u8);
    Ok(codes)
}

fn append_push_value_code(codes: &mut Vec<u8>, value: &Value) -> Rerr {
    use Bytecode::*;
    use Value::*;

    match value {
        Nil => codes.push(PNIL as u8),
        Bool(true) => codes.push(PTRUE as u8),
        Bool(false) => codes.push(PFALSE as u8),
        U8(n) => {
            codes.push(PU8 as u8);
            codes.push(*n);
        }
        U16(n) => {
            codes.push(PU16 as u8);
            codes.extend_from_slice(&n.to_be_bytes());
        }
        U32(n) => {
            append_push_bytes_code(codes, &n.to_be_bytes());
            codes.push(CU32 as u8);
        }
        U64(n) => {
            append_push_bytes_code(codes, &n.to_be_bytes());
            codes.push(CU64 as u8);
        }
        U128(n) => {
            append_push_bytes_code(codes, &n.to_be_bytes());
            codes.push(CU128 as u8);
        }
        Bytes(buf) => append_push_bytes_code(codes, buf),
        Address(addr) => {
            append_push_bytes_code(codes, addr.as_bytes());
            codes.push(CTO as u8);
            codes.push(ValueTy::Address as u8);
        }
        HeapSlice(_) | Tuple(_) | Compo(_) => {
            return errf!("sandbox argument type {:?} not supported", value.ty())
        }
    }
    Ok(())
}

fn append_push_bytes_code(codes: &mut Vec<u8>, bytes: &[u8]) {
    use Bytecode::*;

    if bytes.len() <= u8::MAX as usize {
        codes.push(PBUF as u8);
        codes.push(bytes.len() as u8);
    } else {
        codes.push(PBUFL as u8);
        codes.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    }
    codes.extend_from_slice(bytes);
}

fn parse_one_param(t: &str, v: &str) -> Ret<Value> {
    use ValueTy::*;
    let ty = ValueTy::from_name(t).map_err(|_| format!("unsupported param type '{}'", t))?;
    Ok(match ty {
        Nil => Value::Nil,
        Bool => match v {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return errf!("invalid bool argument '{}'", v),
        },
        U8 => Value::U8(
            v.parse::<u8>()
                .map_err(|e| format!("invalid u8 argument '{}': {}", v, e))?,
        ),
        U16 => Value::U16(
            v.parse::<u16>()
                .map_err(|e| format!("invalid u16 argument '{}': {}", v, e))?,
        ),
        U32 => Value::U32(
            v.parse::<u32>()
                .map_err(|e| format!("invalid u32 argument '{}': {}", v, e))?,
        ),
        U64 => Value::U64(
            v.parse::<u64>()
                .map_err(|e| format!("invalid u64 argument '{}': {}", v, e))?,
        ),
        U128 => Value::U128(
            v.parse::<u128>()
                .map_err(|e| format!("invalid u128 argument '{}': {}", v, e))?,
        ),
        Address => Value::Address(
            field::Address::from_readable(v)
                .map_err(|e| format!("invalid address argument '{}': {}", v, e))?,
        ),
        Bytes => {
            let hex_body = v.strip_prefix("0x").unwrap_or(v);
            Value::Bytes(
                hex::decode(hex_body)
                    .map_err(|e| format!("invalid bytes argument '{}': {}", v, e))?,
            )
        }
        _ => return errf!("unsupported param type '{}'", t),
    })
}

#[cfg(test)]
mod sandbox_parse_tests {
    use super::*;

    #[test]
    fn parse_sandbox_params_accepts_bytes_with_0x_prefix() {
        let args = parse_sandbox_params("0x57495657414b:bytes,0:u16").unwrap();
        assert_eq!(args, vec![Value::Bytes(b"WIVWAK".to_vec()), Value::U16(0)]);
    }

    #[test]
    fn parse_sandbox_params_reports_invalid_bytes() {
        let err = parse_sandbox_params("0xzz:bytes").unwrap_err();
        assert!(err.to_string().contains("invalid bytes argument"));
    }

    #[test]
    fn sandbox_call_codes_reject_function_argv_over_limit() {
        let args = (0..(crate::MAX_FUNC_PARAM_LEN + 1))
            .map(|i| Value::U8(i as u8))
            .collect::<Vec<_>>();
        let err = build_call_codes("f", &args).unwrap_err();
        assert!(err.contains("func argv length cannot more than"));
    }
}
