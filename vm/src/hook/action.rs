pub fn try_action_hook(kid: u16, action: &dyn Any, ctx: &mut dyn Context) -> Rerr {
    use AbstCall::*;

    match kid {
        HacFromToTrs::KIND | HacFromTrs::KIND | HacToTrs::KIND => {
            coin_asset_transfer_call(kid, PermitHAC, PayableHAC, action, ctx)
        }
        SatFromToTrs::KIND | SatFromTrs::KIND | SatToTrs::KIND => {
            coin_asset_transfer_call(kid, PermitSAT, PayableSAT, action, ctx)
        }
        DiaSingleTrs::KIND | DiaFromToTrs::KIND | DiaFromTrs::KIND | DiaToTrs::KIND => {
            coin_asset_transfer_call(kid, PermitHACD, PayableHACD, action, ctx)
        }
        AssetFromToTrs::KIND | AssetFromTrs::KIND | AssetToTrs::KIND => {
            coin_asset_transfer_call(kid, PermitAsset, PayableAsset, action, ctx)
        }
        _ => Ok(()),
    }
}

fn coin_asset_transfer_call(
    kid: u16,
    abstfrom: AbstCall,
    abstto: AbstCall,
    action: &dyn Any,
    ctx: &mut dyn Context,
) -> Rerr {
    let addrs = &ctx.env().tx.addrs;
    let mut from = ctx.env().tx.main;
    let mut to = from.clone();
    let mut argvs: VecDeque<Value>;
    let asset_param = |asset: &AssetAmt| {
        VecDeque::from([
            Value::U64(asset.serial.uint()),
            Value::U64(asset.amount.uint()),
        ])
    };
    macro_rules! diamonds_param {
        ($act: expr) => {
            VecDeque::from([
                Value::U32($act.diamonds.length() as u32),
                Value::Bytes($act.diamonds.form()),
            ])
        };
    }
    // HAC
    if let Some(act) = action.downcast_ref::<HacToTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    } else if let Some(act) = action.downcast_ref::<HacFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    } else if let Some(act) = action.downcast_ref::<HacFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    // SAT
    } else if let Some(act) = action.downcast_ref::<SatToTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    } else if let Some(act) = action.downcast_ref::<SatFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    } else if let Some(act) = action.downcast_ref::<SatFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    // HACD
    } else if let Some(act) = action.downcast_ref::<DiaSingleTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::U32(1), Value::Bytes(act.diamond.to_vec())]);
    } else if let Some(act) = action.downcast_ref::<DiaToTrs>() {
        to = act.to.real(addrs)?;
        argvs = diamonds_param!(act);
    } else if let Some(act) = action.downcast_ref::<DiaFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = diamonds_param!(act);
    } else if let Some(act) = action.downcast_ref::<DiaFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = diamonds_param!(act);
    // Asset
    } else if let Some(act) = action.downcast_ref::<AssetToTrs>() {
        to = act.to.real(addrs)?;
        argvs = asset_param(&act.asset);
    } else if let Some(act) = action.downcast_ref::<AssetFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = asset_param(&act.asset);
    } else if let Some(act) = action.downcast_ref::<AssetFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = asset_param(&act.asset);
    } else {
        unreachable!()
    }

    let (fs, fc, tc) = (from.is_scriptmh(), from.is_contract(), to.is_contract());
    if !(fs || fc || tc) {
        return Ok(()); // no script or contract address
    }

    const P2SH_PARAM_LEN: usize = 5; // witness + kind + to + (arg1, arg2)

    // call from p2sh script
    if fs {
        let p2sh = ctx.p2sh(&from)?;
        let codeconf = p2sh.code_conf();
        let witness = p2sh.witness().to_vec();
        let codes = p2sh.code_stuff().to_vec();
        let mut params: Vec<Value> = Vec::with_capacity(P2SH_PARAM_LEN);
        params.push(Value::Bytes(witness));
        params.push(Value::U16(kid));
        params.push(Value::Address(to));
        let mut args = argvs.clone();
        while params.len() < P2SH_PARAM_LEN {
            match args.pop_front() {
                Some(v) => params.push(v),
                None => params.push(Value::Nil),
            }
        }
        let param = Value::pack_call_args(params)?;
        let _ = setup_vm_run_p2sh(ctx, codeconf, codes, param)?;
        // return value checked inside p2sh_call
    }

    // call from contract abstract
    if fc {
        let mut argvs = argvs.clone();
        argvs.push_front(Value::Address(to));
        let param = Value::pack_call_args(argvs)?;
        let _ = setup_vm_run_abst(ctx, abstfrom, from, param)?;
        // return value checked inside abst_call
    }

    // call to contract abstract
    if tc {
        argvs.push_front(Value::Address(from));
        let param = Value::pack_call_args(argvs)?;
        let _ = setup_vm_run_abst(ctx, abstto, to, param)?;
        // return value checked inside abst_call
    }

    Ok(())
}

#[cfg(test)]
mod hook_arg_tests {
    use super::*;
    use basis::component::{Env, ExecFrom, TexLedger};
    use basis::interface::{Context, GasUse, Logs, P2sh, State, StateOperat, TransactionRead};
    use field::{Address, Amount, Hash};
    use protocol::context::EmptyState;
    use std::sync::{Arc, Weak};
    use sys::{XError, XRet};

    #[derive(Default)]
    struct NoopLogs;
    impl Logs for NoopLogs {}

    #[derive(Default)]
    struct DummyState;
    impl State for DummyState {
        fn fork_sub(&self, _: Weak<Box<dyn State>>) -> Box<dyn State> {
            Box::new(Self)
        }
        fn merge_sub(&mut self, _: Box<dyn State>) {}
        fn detach(&mut self) {}
        fn clone_state(&self) -> Box<dyn State> {
            Box::new(Self)
        }
    }

    #[derive(Debug, Clone, Default)]
    struct DummyTx {
        main: Address,
        addrs: Vec<Address>,
    }

    impl field::Serialize for DummyTx {
        fn size(&self) -> usize {
            0
        }
        fn serialize(&self) -> Vec<u8> {
            vec![]
        }
    }

    impl basis::interface::TxExec for DummyTx {}

    impl TransactionRead for DummyTx {
        fn ty(&self) -> u8 {
            3
        }
        fn hash(&self) -> Hash {
            Hash::default()
        }
        fn hash_with_fee(&self) -> Hash {
            Hash::default()
        }
        fn main(&self) -> Address {
            self.main
        }
        fn addrs(&self) -> Vec<Address> {
            self.addrs.clone()
        }
        fn fee(&self) -> &Amount {
            Amount::zero_ref()
        }
        fn fee_purity(&self) -> u64 {
            1
        }
        fn gas_max_byte(&self) -> Option<u8> {
            Some(1)
        }
    }

    struct DummyP2sh {
        conf: u8,
        code: Vec<u8>,
        witness: Vec<u8>,
    }

    impl P2sh for DummyP2sh {
        fn code_conf(&self) -> u8 {
            self.conf
        }
        fn code_stuff(&self) -> &[u8] {
            &self.code
        }
        fn witness(&self) -> &[u8] {
            &self.witness
        }
    }

    struct CaptureCtx {
        env: Env,
        tx: DummyTx,
        state: Box<dyn State>,
        logs: NoopLogs,
        tex: TexLedger,
        p2sh_addr: Address,
        p2sh_box: Box<dyn P2sh>,
        calls: Vec<(Value, IntentScope)>,
        intent_scope: IntentScope,
        exec_from: ExecFrom,
    }

    impl CaptureCtx {
        fn new(main: Address, p2sh_addr: Address) -> Self {
            let tx = DummyTx {
                main,
                addrs: vec![main, p2sh_addr],
            };
            let mut p2sh_code = Address::default().as_bytes().to_vec();
            p2sh_code.push(0); // empty ContractAddressW1 list
            let mut env = Env::default();
            env.tx.ty = 3;
            env.tx.main = main;
            env.tx.addrs = tx.addrs.clone();
            Self {
                env,
                tx,
                state: Box::new(DummyState),
                logs: NoopLogs,
                tex: TexLedger::default(),
                p2sh_addr,
                p2sh_box: Box::new(DummyP2sh {
                    conf: crate::CodeConf::from_type(crate::CodeType::Bytecode).raw(),
                    code: p2sh_code,
                    witness: vec![0; 21],
                }),
                calls: vec![],
                intent_scope: None,
                exec_from: ExecFrom::Top,
            }
        }

        fn with_intent_scope(mut self, intent_scope: IntentScope) -> Self {
            self.intent_scope = intent_scope;
            self
        }
    }

    impl StateOperat for CaptureCtx {
        fn state(&mut self) -> &mut dyn State {
            self.state.as_mut()
        }
        fn state_fork(&mut self) -> Arc<Box<dyn State>> {
            Arc::new(Box::new(EmptyState {}))
        }
        fn state_merge(&mut self, _: Arc<Box<dyn State>>) {}
        fn state_recover(&mut self, _: Arc<Box<dyn State>>) {}
    }

    impl Context for CaptureCtx {
        fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
            Ok((0, vec![]))
        }
        fn exec_from(&self) -> ExecFrom {
            self.exec_from
        }
        fn exec_from_set(&mut self, from: ExecFrom) {
            self.exec_from = from;
        }
        fn env(&self) -> &Env {
            &self.env
        }
        fn addr(&self, ptr: &AddrOrPtr) -> Ret<Address> {
            ptr.real(&self.env.tx.addrs)
        }
        fn check_sign(&mut self, _: &Address) -> Rerr {
            Ok(())
        }
        fn tx(&self) -> &dyn TransactionRead {
            &self.tx
        }
        fn vm_call(&mut self, req: Box<dyn Any>) -> XRet<(GasUse, Box<dyn Any>)> {
            let Ok(req) = req.downcast::<crate::machine::VmCallReq>() else {
                return Err(XError::fault("vm call req type mismatch".to_owned()));
            };
            let (param, intent_scope) = match *req {
                crate::machine::VmCallReq::Main { .. } => (Value::Nil, None),
                crate::machine::VmCallReq::P2sh { param, intent_binding, .. }
                | crate::machine::VmCallReq::Abst { param, intent_binding, .. } => {
                    (param, intent_binding)
                }
            };
            self.calls.push((param, intent_scope));
            Ok((GasUse { compute: 1, resource: 0, storage: 0 }, Box::new(Value::Nil)))
        }
        fn vm_current_intent_scope(&mut self) -> IntentScope {
            self.intent_scope
        }
        fn tex_ledger(&mut self) -> &mut TexLedger {
            &mut self.tex
        }
        fn logs(&mut self) -> &mut dyn Logs {
            &mut self.logs
        }
        fn p2sh(&self, addr: &Address) -> Ret<&dyn P2sh> {
            if *addr == self.p2sh_addr {
                Ok(self.p2sh_box.as_ref())
            } else {
                errf!("not found")
            }
        }
    }

    #[test]
    fn action_hook_packs_multi_arg_p2sh_call_as_tuple() {
        let p2sh_addr = Address::create_scriptmh([2u8; 20]);
        let to = Address::create_privakey([1u8; 20]);
        let mut ctx = CaptureCtx::new(p2sh_addr, p2sh_addr);

        let act = HacToTrs::create_by(to, Amount::mei(1));

        try_action_hook(HacToTrs::KIND, &act, &mut ctx).unwrap();
        assert_eq!(ctx.calls.len(), 1);
        let param = &ctx.calls[0].0;
        assert!(matches!(param, Value::Tuple(_)));
    }

    #[test]
    fn action_hook_packs_abst_call_as_tuple() {
        let main = Address::create_privakey([1u8; 20]);
        let contract = Address::create_contract([3u8; 20]);
        let mut ctx = CaptureCtx::new(main, Address::create_scriptmh([2u8; 20]));

        let act = HacToTrs::create_by(contract, Amount::mei(1));

        try_action_hook(HacToTrs::KIND, &act, &mut ctx).unwrap();
        assert_eq!(ctx.calls.len(), 1);
        let param = &ctx.calls[0].0;
        assert!(matches!(param, Value::Tuple(_)));
    }

    #[test]
    fn action_hook_carries_current_intent_scope_into_vm_callback() {
        let main = Address::create_privakey([1u8; 20]);
        let contract = Address::create_contract([3u8; 20]);
        let mut ctx = CaptureCtx::new(main, Address::create_scriptmh([2u8; 20]))
            .with_intent_scope(Some(Some(77)));

        let act = HacToTrs::create_by(contract, Amount::mei(1));

        try_action_hook(HacToTrs::KIND, &act, &mut ctx).unwrap();
        assert_eq!(ctx.calls.len(), 1);
        assert_eq!(ctx.calls[0].1, Some(Some(77)));
    }
}
