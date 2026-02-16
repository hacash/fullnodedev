mod common;

#[cfg(test)]
mod action_coverage {
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard, Once, OnceLock};

    use basis::component::{Env, ACTION_CTX_LEVEL_CALL_CONTRACT, ACTION_CTX_LEVEL_CALL_MAIN};
    use basis::interface::ActExec;
    use basis::interface::{ActCall, Context, Logs, State, StateOperat, Transaction, TransactionRead};
    use field::{
        Address, Amount, BytesW1, BytesW2, DiamondName, DiamondSto, Field, Hash, Inscripts, Parse,
        Readable, Serialize, Uint1, Uint2, Uint4,
    };
    use protocol::context::ContextInst;
    use protocol::state::CoreState;
    use protocol::transaction::{create_tx_info, TransactionType3};
    use sys::{Account, Ret};
    use vm::action::*;
    use vm::build_codes;
    use vm::contract::{Abst, Contract, Func};
    use vm::frame::ExecEnv;
    use vm::lang::lang_to_bytecode;
    use vm::machine::{self, Machine, Resoure};
    use vm::rt::{AbstCall, Bytecode, Bytecode::*, CodeType, SpaceCap, calc_func_sign};
    use vm::value::Value;
    use vm::{ContractAddress, ContractAddressW1, ContractAddrsssW1, ContractSto, VMState};

    // ─── Test infrastructure ───

    #[derive(Default, Clone)]
    struct StateMem {
        mem: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl State for StateMem {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            self.mem.get(&k).cloned()
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.insert(k, v);
        }
        fn del(&mut self, k: Vec<u8>) {
            self.mem.remove(&k);
        }
    }

    #[derive(Default, Clone)]
    struct MemLogs {
        items: Vec<Vec<u8>>,
    }

    impl Logs for MemLogs {
        fn push(&mut self, stuff: &dyn Serialize) {
            self.items.push(stuff.serialize());
        }
        fn load(&self, _height: u64, idx: usize) -> Option<Vec<u8>> {
            self.items.get(idx).cloned()
        }
        fn remove(&self, _height: u64) {}
        fn snapshot_len(&self) -> usize { self.items.len() }
        fn truncate(&mut self, len: usize) { self.items.truncate(len); }
    }

    #[derive(Clone, Debug)]
    struct TestTx {
        ty: u8,
        main: Address,
        addrs: Vec<Address>,
        fee: Amount,
        gas_max: u8,
        tx_size: usize,
    }

    impl Serialize for TestTx {
        fn size(&self) -> usize { self.tx_size }
        fn serialize(&self) -> Vec<u8> { vec![] }
    }

    impl basis::interface::TxExec for TestTx {}

    impl TransactionRead for TestTx {
        fn ty(&self) -> u8 { self.ty }
        fn hash(&self) -> Hash { Hash::default() }
        fn hash_with_fee(&self) -> Hash { Hash::default() }
        fn main(&self) -> Address { self.main }
        fn addrs(&self) -> Vec<Address> { self.addrs.clone() }
        fn fee(&self) -> &Amount { &self.fee }
        fn fee_got(&self) -> Amount { self.fee.clone() }
        fn fee_purity(&self) -> u64 { 3200 }
        fn fee_extend(&self) -> Ret<u8> { Ok(self.gas_max) }
    }

    fn test_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn init_action_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            protocol::setup::action_register(protocol::action::try_create, protocol::action::try_json_decode);
            protocol::setup::action_register(vm::action::try_create, vm::action::try_json_decode);
        });
    }

    fn main_addr() -> Address {
        Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
    }

    fn alt_addr() -> Address {
        Address::from_readable("1EuGe2GU8tDKnHLNfBsgyffx66buK7PP6g").unwrap()
    }

    fn contract_addr(main: &Address, nonce: u32) -> ContractAddress {
        ContractAddress::calculate(main, &Uint4::from(nonce))
    }

    fn make_tx(ty: u8, main: Address, addrs: Vec<Address>, gas_max: u8) -> TestTx {
        TestTx {
            ty, main, addrs,
            fee: Amount::unit238(10_000_000),
            gas_max,
            tx_size: 128,
        }
    }

    fn make_ctx_from_tx<'a>(height: u64, tx: &'a dyn TransactionRead, state: Box<dyn State>, logs: Box<dyn Logs>) -> ContextInst<'a> {
        let mut env = Env::default();
        env.block.height = height;
        env.tx = create_tx_info(tx);
        ContextInst::new(env, state, logs, tx)
    }

    fn make_ctx<'a>(height: u64, tx: &'a TestTx, state: Box<dyn State>, logs: Box<dyn Logs>) -> ContextInst<'a> {
        make_ctx_from_tx(height, tx, state, logs)
    }

    fn make_tx3(main: Address, gas_max: u8) -> TransactionType3 {
        let mut tx = TransactionType3::new_by(main, Amount::unit238(10_000_000), 1_730_000_000);
        tx.gas_max = Uint1::from(gas_max);
        tx
    }

    fn insert_contract(state: &mut dyn State, addr: &ContractAddress, sto: &ContractSto) {
        VMState::wrap(state).contract_set(addr, sto);
    }

    fn make_public_contract(func_name: &str, body: &str) -> ContractSto {
        Contract::new()
            .func(Func::new(func_name).unwrap().public().fitsh(body).unwrap())
            .into_sto()
    }

    fn execute_main_bytecode(ctx: &mut dyn Context, codes: Vec<u8>) -> Ret<Value> {
        let mut gas = 1_000_000i64;
        let height = ctx.env().block.height;
        let mut machine = Machine::create(Resoure::create(height));
        let mut exenv = ExecEnv { ctx, gas: &mut gas };
        machine.main_call(&mut exenv, CodeType::Bytecode, codes.into())
    }

    fn execute_main_bytecode_as_call_ctx(ctx: &mut dyn Context, codes: Vec<u8>) -> Ret<Value> {
        // `Machine::main_call` itself does not mutate ctx.level; simulate protocol's call-level setup.
        let old_level = ctx.level();
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);
        let res = execute_main_bytecode(ctx, codes);
        ctx.level_set(old_level);
        res
    }

    fn single_call_codes(lib_idx: u8, sig: [u8; 4]) -> Vec<u8> {
        let mut codes = vec![Bytecode::PNIL as u8, Bytecode::CALL as u8, lib_idx];
        codes.extend_from_slice(&sig);
        codes.push(Bytecode::END as u8);
        codes
    }

    fn callsuper_codes(sig: [u8; 4]) -> Vec<u8> {
        let mut codes = vec![Bytecode::PNIL as u8, Bytecode::CALLSUPER as u8];
        codes.extend_from_slice(&sig);
        codes.push(Bytecode::END as u8);
        codes
    }

    fn set_vm_assigner(assigner: Option<protocol::setup::FnVmAssignFunc>) {
        unsafe { protocol::setup::VM_ASSIGN_FUNC = assigner; }
    }

    fn execute_deploy(ctx: &mut dyn Context, nonce: u32, contract: ContractSto) -> Ret<(u32, Vec<u8>)> {
        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(nonce);
        // Provide generous protocol fee to cover any contract size
        act.protocol_cost = Amount::coin(10000, 244);
        act.contract = contract;
        act.execute(ctx)
    }

    fn fund_main_addr(ctx: &mut dyn Context) {
        let main = ctx.env().tx.main;
        protocol::operate::hac_add(ctx, &main, &Amount::mei(1000)).unwrap();
    }

    // ═══════════════════════════════════════════════════
    // TxMessage tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn tx_message_execute_returns_empty() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = TxMessage::new();
        act.data = BytesW1::from(b"hello world".to_vec()).unwrap();
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0, "gas should be charged");
        assert!(res.is_empty(), "TxMessage should return empty vec");
    }

    #[test]
    fn tx_message_serialize_roundtrip() {
        let mut act = TxMessage::new();
        act.data = BytesW1::from(b"test data 123".to_vec()).unwrap();
        let bytes = act.serialize();
        let mut act2 = TxMessage::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn tx_message_empty_data() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = TxMessage::new(); // empty data
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn tx_message_top_unique_rejects_duplicate_actions_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        tx.push_action(Box::new(TxMessage::new())).unwrap();

        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        // keep fast_sync = false to exercise action level checks
        let err = TxMessage::new().execute(&mut ctx).unwrap_err();
        assert!(err.contains("TOP_UNIQUE"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // TxBlob tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn tx_blob_execute_returns_empty() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = TxBlob::new();
        act.data = BytesW2::from(vec![0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        assert!(res.is_empty(), "TxBlob should return empty vec");
    }

    #[test]
    fn tx_blob_serialize_roundtrip() {
        let mut act = TxBlob::new();
        act.data = BytesW2::from(vec![1, 2, 3, 4, 5]).unwrap();
        let bytes = act.serialize();
        let mut act2 = TxBlob::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn tx_blob_large_data() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let large_data = vec![0xAB; 1024];
        let mut act = TxBlob::new();
        act.data = BytesW2::from(large_data).unwrap();
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn tx_blob_top_unique_rejects_non_top_context_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxBlob::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let err = TxBlob::new().execute(&mut ctx).unwrap_err();
        assert!(err.contains("TOP_UNIQUE"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // ContractMainCall tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn maincall_from_bytecode_end_returns_zero() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let codes = lang_to_bytecode("return 0").unwrap();
        let rv = execute_main_bytecode(&mut ctx, codes).unwrap();
        assert!(!rv.check_true(), "return 0 should yield falsy value");
    }

    #[test]
    fn maincall_nonzero_return_is_error() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let codes = lang_to_bytecode("return 1").unwrap();
        let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
        assert!(err.contains("main call return error code 1"), "{err}");
    }

    #[test]
    fn maincall_rejects_invalid_code_type() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractMainCall::new();
        act.ctype = Uint1::from(99); // invalid code type
        act.codes = BytesW2::from(vec![Bytecode::END as u8]).unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.to_lowercase().contains("code type") || err.contains("CodeType"), "{err}");
    }

    #[test]
    fn maincall_rejects_nonzero_marks() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractMainCall::new();
        act.ctype = Uint1::from(0);
        act.codes = BytesW2::from(vec![Bytecode::END as u8]).unwrap();
        // Set marks to non-zero
        let bytes = act.serialize();
        let mut act2 = ContractMainCall::new();
        act2.parse(&bytes).unwrap();
        // Manually set marks field to non-zero via raw parse
        let mut raw = act2.serialize();
        // kind(2) + marks(3) + ctype(1) + codes(2+len)
        // marks starts at offset 2
        raw[2] = 0xFF;
        let mut act3 = ContractMainCall::new();
        act3.parse(&raw).unwrap();
        let err = act3.execute(&mut ctx).unwrap_err();
        assert!(err.contains("marks"), "{err}");
    }

    #[test]
    fn maincall_serialize_roundtrip() {
        let codes = lang_to_bytecode("return 0").unwrap();
        let act = ContractMainCall::from_bytecode(codes).unwrap();
        let bytes = act.serialize();
        let mut act2 = ContractMainCall::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn maincall_calls_contract_function() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 5001);
        let sto = make_public_contract("add1", r#"
            param { n }
            assert n == 5
            return 0
        "#);
        let mut state = StateMem::default();
        insert_contract(&mut state, &caddr, &sto);

        let tx = make_tx(3, main, vec![caddr.to_addr()], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));

        let sig = calc_func_sign("add1");
        // push param 5 as u8, then call lib 0 function
        let mut codes = vec![
            Bytecode::PU8 as u8, 5,
            Bytecode::CALL as u8, 0,
        ];
        codes.extend_from_slice(&sig);
        codes.push(Bytecode::END as u8);
        execute_main_bytecode(&mut ctx, codes).unwrap();
    }

    #[test]
    fn maincall_ast_level_rejects_main_call_context_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let act = ContractMainCall::from_bytecode(lang_to_bytecode("return 0").unwrap()).unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("max ctx level"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // ContractDeploy tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn deploy_simple_contract_succeeds() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        let sto = make_public_contract("f", "return 0");
        execute_deploy(&mut ctx, 1, sto).unwrap();

        // Verify contract exists in state
        let caddr = contract_addr(&main, 1);
        let loaded = VMState::wrap(StateOperat::state(&mut ctx)).contract(&caddr);
        assert!(loaded.is_some(), "deployed contract should exist in state");
    }

    #[test]
    fn deploy_rejects_duplicate_address() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut state = StateMem::default();
        let caddr = contract_addr(&main, 1);
        let sto = make_public_contract("f", "return 0");
        insert_contract(&mut state, &caddr, &sto);

        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("already exist"), "{err}");
    }

    #[test]
    fn deploy_rejects_self_inherit() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sto = Contract::new()
            .inh(caddr.to_addr())
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("inherit itself"), "{err}");
    }

    #[test]
    fn deploy_rejects_self_library() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sto = Contract::new()
            .lib(caddr.to_addr())
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("link itself as library"), "{err}");
    }

    #[test]
    fn deploy_rejects_nonzero_revision() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut sto = make_public_contract("f", "return 0");
        sto.metas.revision = Uint2::from(1u16);
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("revision must be 0"), "{err}");
    }

    #[test]
    fn deploy_rejects_missing_inherit_contract() {
        let _guard = test_guard();
        let main = main_addr();
        let missing = contract_addr(&main, 9999);
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sto = Contract::new()
            .inh(missing.to_addr())
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("not exist"), "{err}");
    }

    #[test]
    fn deploy_with_construct_function() {
        let _guard = test_guard();
        set_vm_assigner(Some(machine::vm_assign));
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        let sto = Contract::new()
            .syst(
                Abst::new(AbstCall::Construct)
                    .bytecode(build_codes!(CU8 RET))
                    .unwrap(),
            )
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        execute_deploy(&mut ctx, 100, sto).unwrap();

        let caddr = contract_addr(&main, 100);
        assert!(VMState::wrap(StateOperat::state(&mut ctx)).contract(&caddr).is_some());
        set_vm_assigner(None);
    }

    #[test]
    fn deploy_top_only_with_guard_rejects_multiple_non_guard_actions() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        tx.push_action(Box::new(TxBlob::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(1u32);
        act.protocol_cost = Amount::zero();
        act.contract = make_public_contract("f", "return 0");
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("TOP_ONLY_WITH_GUARD"), "{err}");
    }

    #[test]
    fn deploy_rejects_missing_library_contract() {
        let _guard = test_guard();
        let main = main_addr();
        let missing_lib = contract_addr(&main, 9009);
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sto = Contract::new()
            .lib(missing_lib.to_addr())
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("library") && err.contains("not exist"), "{err}");
    }

    #[test]
    fn deploy_rejects_inherits_cycle_in_existing_graph() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut state = StateMem::default();
        let a = contract_addr(&main, 2);
        let b = contract_addr(&main, 3);
        let sto_a = Contract::new()
            .inh(b.to_addr())
            .func(Func::new("fa").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let sto_b = Contract::new()
            .inh(a.to_addr())
            .func(Func::new("fb").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        insert_contract(&mut state, &a, &sto_a);
        insert_contract(&mut state, &b, &sto_b);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let root = Contract::new()
            .inh(a.to_addr())
            .func(Func::new("f").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, root).unwrap_err();
        assert!(err.contains("inherits cyclic detected"), "{err}");
    }

    #[test]
    fn deploy_rejects_static_call_libidx_overflow() {
        let _guard = test_guard();
        let main = main_addr();
        let lib_addr = contract_addr(&main, 88);
        let tx = make_tx(3, main, vec![], 17);
        let mut state = StateMem::default();
        insert_contract(&mut state, &lib_addr, &make_public_contract("target", "return 0"));
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let target_sig = calc_func_sign("target");
        let codes = single_call_codes(1, target_sig); // overflow: only libidx 0 exists
        let sto = Contract::new()
            .lib(lib_addr.to_addr())
            .func(Func::new("f").unwrap().public().bytecode(codes).unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("libidx overflow"), "{err}");
    }

    #[test]
    fn deploy_rejects_static_call_missing_library_function() {
        let _guard = test_guard();
        let main = main_addr();
        let lib_addr = contract_addr(&main, 89);
        let tx = make_tx(3, main, vec![], 17);
        let mut state = StateMem::default();
        insert_contract(&mut state, &lib_addr, &make_public_contract("have", "return 0"));
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let miss_sig = calc_func_sign("missing");
        let sto = Contract::new()
            .lib(lib_addr.to_addr())
            .func(Func::new("f").unwrap().public().bytecode(single_call_codes(0, miss_sig)).unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("function") && err.contains("not found"), "{err}");
    }

    #[test]
    fn deploy_rejects_callsuper_missing_function() {
        let _guard = test_guard();
        let main = main_addr();
        let parent_addr = contract_addr(&main, 66);
        let tx = make_tx(3, main, vec![], 17);
        let mut state = StateMem::default();
        insert_contract(&mut state, &parent_addr, &make_public_contract("have", "return 0"));
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let miss_sig = calc_func_sign("missing");
        let sto = Contract::new()
            .inh(parent_addr.to_addr())
            .func(Func::new("f").unwrap().public().bytecode(callsuper_codes(miss_sig)).unwrap())
            .into_sto();
        let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
        assert!(err.contains("super function") && err.contains("not found"), "{err}");
    }

    #[test]
    fn deploy_rejects_construct_argv_size_overflow() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(77u32);
        act.protocol_cost = Amount::coin(10000, 244);
        let over = SpaceCap::new(1).max_value_size + 1;
        act.construct_argv = BytesW2::from(vec![0xAB; over]).unwrap();
        act.contract = make_public_contract("f", "return 0");

        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("construct argv size overflow"), "{err}");
    }

    #[test]
    fn deploy_accepts_construct_argv_at_spacecap_limit() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(78u32);
        act.protocol_cost = Amount::coin(10000, 244);
        let cap = SpaceCap::new(1).max_value_size;
        act.construct_argv = BytesW2::from(vec![0xCD; cap]).unwrap();
        act.contract = make_public_contract("f", "return 0");
        act.execute(&mut ctx).unwrap();

        let caddr = contract_addr(&main, 78);
        assert!(VMState::wrap(StateOperat::state(&mut ctx)).contract(&caddr).is_some());
    }

    // ═══════════════════════════════════════════════════
    // ContractUpdate tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn update_rejects_nonexistent_contract() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let caddr = contract_addr(&main, 1);
        let mut act = ContractUpdate::new();
        act.address = caddr.to_addr();
        act.protocol_cost = Amount::zero();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("not exist"), "{err}");
    }

    #[test]
    fn update_rejects_nonzero_marks() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let sto = make_public_contract("f", "return 0");
        let mut state = StateMem::default();
        insert_contract(&mut state, &caddr, &sto);

        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractUpdate::new();
        act.address = caddr.to_addr();
        act.protocol_cost = Amount::zero();
        // Serialize, find marks field, corrupt it
        let raw = act.serialize();
        // Find the _marks_ field: it's Fixed2 right after address
        // Layout: kind(2) + protocol_cost(serialized) + address(21) + _marks_(2) + edit(...)
        // Amount::zero() serializes to 1 byte (0x00)
        // So marks is at offset 2 + 1 + 21 = 24
        let marks_offset = 2 + Amount::zero().size() + 21;
        let mut corrupted = raw.clone();
        corrupted[marks_offset] = 0xFF;
        let mut act2 = ContractUpdate::new();
        act2.parse(&corrupted).unwrap();
        let err = act2.execute(&mut ctx).unwrap_err();
        assert!(err.contains("marks"), "{err}");
    }

    #[test]
    fn update_rejects_self_inherit_after_edit() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let sto = make_public_contract("f", "return 0");
        let mut state = StateMem::default();
        insert_contract(&mut state, &caddr, &sto);

        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractUpdate::new();
        act.address = caddr.to_addr();
        act.protocol_cost = Amount::zero();
        // Add self as inherit via edit
        act.edit.inherits_add = ContractAddrsssW1::from_list(
            vec![caddr.clone()]
        ).unwrap();
        // Revision 0 -> next is 1
        act.edit.expect_revision = Uint2::from(1u16);
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("inherit itself"), "{err}");
    }

    #[test]
    fn update_rejects_missing_library_contract_after_edit() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let missing_lib = contract_addr(&main, 9999);
        let mut state = StateMem::default();
        insert_contract(&mut state, &caddr, &make_public_contract("f", "return 0"));

        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractUpdate::new();
        act.address = caddr.to_addr();
        act.protocol_cost = Amount::zero();
        act.edit.expect_revision = Uint2::from(1u16);
        act.edit.librarys_add = ContractAddrsssW1::from_list(vec![missing_lib]).unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("library") && err.contains("not exist"), "{err}");
    }

    #[test]
    fn update_rejects_inherits_cycle_after_edit() {
        let _guard = test_guard();
        let main = main_addr();
        let root = contract_addr(&main, 1);
        let parent = contract_addr(&main, 2);
        let mut state = StateMem::default();
        // root has no inherit; parent inherits root
        insert_contract(&mut state, &root, &make_public_contract("rootf", "return 0"));
        insert_contract(
            &mut state,
            &parent,
            &Contract::new()
                .inh(root.to_addr())
                .func(Func::new("pf").unwrap().public().fitsh("return 0").unwrap())
                .into_sto(),
        );

        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ContractUpdate::new();
        act.address = root.to_addr();
        act.protocol_cost = Amount::zero();
        act.edit.expect_revision = Uint2::from(1u16);
        act.edit.inherits_add = ContractAddrsssW1::from_list(vec![parent]).unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("inherits cyclic detected"), "{err}");
    }

    #[test]
    fn update_rejects_static_call_libidx_overflow_after_edit() {
        let _guard = test_guard();
        let main = main_addr();
        let caddr = contract_addr(&main, 1);
        let mut state = StateMem::default();
        insert_contract(&mut state, &caddr, &make_public_contract("f", "return 0"));

        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sig = calc_func_sign("target");
        let edit = Contract::new()
            .func(Func::new("g").unwrap().public().bytecode(single_call_codes(0, sig)).unwrap())
            .into_edit(1);

        let mut act = ContractUpdate::new();
        act.address = caddr.to_addr();
        act.protocol_cost = Amount::zero();
        act.edit = edit;
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("libidx overflow"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // EnvMainAddr tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn env_main_addr_returns_tx_main() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(100, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = EnvMainAddr::new();
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        assert_eq!(res, main.to_vec(), "EnvMainAddr should return main address bytes");
    }

    #[test]
    fn env_main_addr_different_addresses() {
        let _guard = test_guard();
        let addr = alt_addr();
        let tx = make_tx(3, addr, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = EnvMainAddr::new();
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, addr.to_vec());
    }

    #[test]
    fn env_main_addr_rejects_top_level_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        // keep level as TOP and fast_sync=false
        let err = EnvMainAddr::new().execute(&mut ctx).unwrap_err();
        assert!(err.contains("VM call context"), "{err}");
    }

    #[test]
    fn env_main_addr_allows_vm_call_level_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let (_, res) = EnvMainAddr::new().execute(&mut ctx).unwrap();
        assert_eq!(res, main.to_vec());
    }

    #[test]
    fn ctx_action_call_env_main_addr_rejects_top_level_when_not_fast_sync() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let err = ctx.action_call(EnvMainAddr::KIND, vec![]).unwrap_err();
        assert!(err.contains("VM call context"), "{err}");
    }

    #[test]
    fn ctx_action_call_env_main_addr_allows_call_level_when_not_fast_sync() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let (_, res) = ctx.action_call(EnvMainAddr::KIND, vec![]).unwrap();
        assert_eq!(res, main.to_vec());
    }

    // ═══════════════════════════════════════════════════
    // EnvCoinbaseAddr tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn env_coinbase_addr_returns_block_coinbase() {
        let _guard = test_guard();
        let main = main_addr();
        let coinbase = alt_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        ctx.env.block.coinbase = coinbase;

        let act = EnvCoinbaseAddr::new();
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        assert_eq!(res, coinbase.to_vec(), "EnvCoinbaseAddr should return coinbase address");
    }

    #[test]
    fn env_coinbase_addr_default_is_zero() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = EnvCoinbaseAddr::new();
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, Address::default().to_vec());
    }

    // ═══════════════════════════════════════════════════
    // EnvHeight tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn env_height_returns_block_height() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(12345, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = EnvHeight::new();
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        assert_eq!(res, 12345u64.to_be_bytes().to_vec());
    }

    #[test]
    fn env_height_zero() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(0, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let act = EnvHeight::new();
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, 0u64.to_be_bytes().to_vec());
    }

    #[test]
    fn env_height_allows_contract_call_level_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(55, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_CONTRACT);

        let (_, res) = EnvHeight::new().execute(&mut ctx).unwrap();
        assert_eq!(res, 55u64.to_be_bytes().to_vec());
    }

    #[test]
    fn extenv_actions_work_in_vm_call_context() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let coinbase = alt_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(777, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.block.coinbase = coinbase;

        let script = format!(
            r#"
            var h = block_height()
            assert h == 777
            var m = tx_main_address()
            assert m == {main}
            var cb = block_coinbase_address()
            assert cb == {coinbase}
            return 0
            "#,
            main = main.to_readable(),
            coinbase = coinbase.to_readable(),
        );
        let codes = lang_to_bytecode(&script).unwrap();
        assert!(codes.contains(&(Bytecode::EXTENV as u8)));

        let rv = execute_main_bytecode_as_call_ctx(&mut ctx, codes).unwrap();
        assert!(!rv.check_true());
    }

    // ═══════════════════════════════════════════════════
    // ViewCheckSign tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn view_check_sign_unsigned_returns_zero() {
        let _guard = test_guard();
        let main = main_addr();
        let target = alt_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ViewCheckSign::new();
        act.addr = target;
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        // target is not in addrs and has no signature, should return 0
        assert_eq!(res, vec![0], "unsigned address should return [0]");
    }

    #[test]
    fn view_check_sign_signed_main_returns_one() {
        let _guard = test_guard();
        let main_acc = Account::create_by("vm-action-coverage-main-signer").unwrap();
        let main = Address::from(*main_acc.address());
        let mut tx = make_tx3(main, 17);
        tx.fill_sign(&main_acc).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ViewCheckSign::new();
        act.addr = main;
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, vec![1], "signed address should return [1]");
    }

    #[test]
    fn view_check_sign_serialize_roundtrip() {
        let mut act = ViewCheckSign::new();
        act.addr = main_addr();
        let bytes = act.serialize();
        let mut act2 = ViewCheckSign::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn view_check_sign_rejects_top_level_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let mut act = ViewCheckSign::new();
        act.addr = main;
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("VM call context"), "{err}");
    }

    #[test]
    fn ctx_action_call_view_check_sign_rejects_top_level_when_not_fast_sync() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let mut act = ViewCheckSign::new();
        act.addr = main;
        let body = act.serialize()[2..].to_vec();
        let err = ctx.action_call(ViewCheckSign::KIND, body).unwrap_err();
        assert!(err.contains("VM call context"), "{err}");
    }

    #[test]
    fn ctx_action_call_view_check_sign_allows_call_level_and_returns_zero() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let mut act = ViewCheckSign::new();
        act.addr = main;
        let body = act.serialize()[2..].to_vec();
        let (_, res) = ctx.action_call(ViewCheckSign::KIND, body).unwrap();
        assert_eq!(res, vec![0]);
    }

    #[test]
    fn extview_actions_work_in_vm_call_context() {
        let _guard = test_guard();
        init_action_registry();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));

        let script = format!(
            r#"
            var ok = check_signature({main})
            assert ok == false
            var b = balance({main})
            assert size(b) >= 12
            return 0
            "#,
            main = main.to_readable(),
        );
        let codes = lang_to_bytecode(&script).unwrap();
        assert!(codes.contains(&(Bytecode::EXTVIEW as u8)));

        let rv = execute_main_bytecode_as_call_ctx(&mut ctx, codes).unwrap();
        assert!(!rv.check_true());
    }

    // ═══════════════════════════════════════════════════
    // ViewBalance tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn view_balance_empty_account_returns_zeros() {
        let _guard = test_guard();
        let main = main_addr();
        let target = alt_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ViewBalance::new();
        act.addr = target;
        let (gas, res) = act.execute(&mut ctx).unwrap();
        assert!(gas > 0);
        // Result: 4 bytes diamond + 8 bytes satoshi + Amount bytes for hacash
        assert!(res.len() >= 12, "balance result should be at least 12 bytes, got {}", res.len());
        // Diamond count should be 0
        let diamond_count = u32::from_be_bytes([res[0], res[1], res[2], res[3]]);
        assert_eq!(diamond_count, 0);
        // Satoshi should be 0
        let satoshi = u64::from_be_bytes([res[4], res[5], res[6], res[7], res[8], res[9], res[10], res[11]]);
        assert_eq!(satoshi, 0);
    }

    #[test]
    fn view_balance_with_hacash() {
        let _guard = test_guard();
        let main = main_addr();
        let target = alt_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        protocol::operate::hac_add(&mut ctx, &target, &Amount::mei(100)).unwrap();
        ctx.env.chain.fast_sync = true;

        let mut act = ViewBalance::new();
        act.addr = target;
        let (_, res) = act.execute(&mut ctx).unwrap();
        // Diamond count = 0
        let diamond_count = u32::from_be_bytes([res[0], res[1], res[2], res[3]]);
        assert_eq!(diamond_count, 0);
        // Satoshi = 0
        let satoshi = u64::from_be_bytes([res[4], res[5], res[6], res[7], res[8], res[9], res[10], res[11]]);
        assert_eq!(satoshi, 0);
        // HAC amount should be non-zero (100 mei)
        let hac_bytes = &res[12..];
        assert!(!hac_bytes.iter().all(|&b| b == 0), "HAC balance should be non-zero");
    }

    #[test]
    fn view_balance_serialize_roundtrip() {
        let mut act = ViewBalance::new();
        act.addr = main_addr();
        let bytes = act.serialize();
        let mut act2 = ViewBalance::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    // ═══════════════════════════════════════════════════
    // ViewDiamondInscNum tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn view_diamond_insc_num_missing_diamond_errors() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ViewDiamondInscNum::new();
        act.diamond = DiamondName::from_readable(b"AAAAAA").unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("not find"), "{err}");
    }

    #[test]
    fn view_diamond_insc_num_serialize_roundtrip() {
        let mut act = ViewDiamondInscNum::new();
        act.diamond = DiamondName::from_readable(b"ABCDEF").unwrap();
        let bytes = act.serialize();
        let mut act2 = ViewDiamondInscNum::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn view_diamond_insc_num_returns_count() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let diamond = DiamondName::from_readable(b"ABCDEF").unwrap();
        let mut dia = DiamondSto::new();
        dia.address = main;
        dia.inscripts = Inscripts::from_list(vec![
            BytesW1::from_str("insc-1").unwrap(),
            BytesW1::from_str("insc-2").unwrap(),
        ]).unwrap();
        CoreState::wrap(StateOperat::state(&mut ctx)).diamond_set(&diamond, &dia);

        let mut act = ViewDiamondInscNum::new();
        act.diamond = diamond;
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, vec![2]);
    }

    // ═══════════════════════════════════════════════════
    // ViewDiamondInscGet tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn view_diamond_insc_get_missing_diamond_errors() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = ViewDiamondInscGet::new();
        act.diamond = DiamondName::from_readable(b"AAAAAA").unwrap();
        act.inscidx = Uint1::from(0);
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("not find"), "{err}");
    }

    #[test]
    fn view_diamond_insc_get_returns_selected_inscription() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let diamond = DiamondName::from_readable(b"FEDCBA").unwrap();
        let mut dia = DiamondSto::new();
        dia.address = main;
        dia.inscripts = Inscripts::from_list(vec![
            BytesW1::from_str("first").unwrap(),
            BytesW1::from_str("second").unwrap(),
        ]).unwrap();
        CoreState::wrap(StateOperat::state(&mut ctx)).diamond_set(&diamond, &dia);

        let mut act = ViewDiamondInscGet::new();
        act.diamond = diamond;
        act.inscidx = Uint1::from(1);
        let (_, res) = act.execute(&mut ctx).unwrap();
        assert_eq!(res, b"second".to_vec());
    }

    #[test]
    fn view_diamond_insc_get_index_overflow_errors() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let diamond = DiamondName::from_readable(b"AABBCC").unwrap();
        let mut dia = DiamondSto::new();
        dia.address = main;
        dia.inscripts = Inscripts::from_list(vec![BytesW1::from_str("only").unwrap()]).unwrap();
        CoreState::wrap(StateOperat::state(&mut ctx)).diamond_set(&diamond, &dia);

        let mut act = ViewDiamondInscGet::new();
        act.diamond = diamond;
        act.inscidx = Uint1::from(1); // overflow: only idx 0 exists
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("overflow"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // UnlockScriptProve tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn unlock_script_prove_rejects_nonzero_marks() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let mut act = UnlockScriptProve::new();
        act.lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        act.argvkey = BytesW2::from(vec![]).unwrap();
        // Serialize and corrupt marks
        let mut raw = act.serialize();
        // Find marks position and set to non-zero
        // The struct layout: kind(2) + argvkey(2+len) + lockbox(2+len) + adrlibs(1+...) + merkels(1+...) + marks(2)
        // marks is the last field, 2 bytes before end
        let len = raw.len();
        raw[len - 2] = 0xFF;
        let mut act2 = UnlockScriptProve::new();
        act2.parse(&raw).unwrap();
        let err = act2.execute(&mut ctx).unwrap_err();
        assert!(err.contains("marks"), "{err}");
    }

    #[test]
    fn unlock_script_prove_ast_level_rejects_main_call_context_when_not_fast_sync() {
        let _guard = test_guard();
        let main = main_addr();
        let mut tx = make_tx3(main, 17);
        tx.push_action(Box::new(TxMessage::new())).unwrap();
        let mut ctx = make_ctx_from_tx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);

        let mut act = UnlockScriptProve::new();
        act.lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        act.argvkey = BytesW2::from(vec![]).unwrap();
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("max ctx level"), "{err}");
    }

    #[test]
    fn unlock_script_prove_serialize_roundtrip() {
        let mut act = UnlockScriptProve::new();
        act.lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 42, Bytecode::END as u8]).unwrap();
        act.argvkey = BytesW2::from(vec![1, 2, 3]).unwrap();
        let bytes = act.serialize();
        let mut act2 = UnlockScriptProve::new();
        act2.parse(&bytes).unwrap();
        assert_eq!(act, act2);
    }

    #[test]
    fn unlock_script_prove_calc_scriptmh_single_leaf() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        let empty_merkels = MerkelStuffs::from_list(vec![]).unwrap();

        let calc = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox, &empty_merkels).unwrap();
        // Single leaf: sha3_path should have exactly 1 entry (the leaf hash = root hash)
        assert_eq!(calc.sha3_path.len(), 1);
        // Address should be a valid scriptmh address
        assert!(calc.address.is_scriptmh(), "address should be scriptmh type");
    }

    #[test]
    fn unlock_script_prove_calc_scriptmh_deterministic() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 99, Bytecode::END as u8]).unwrap();
        let empty_merkels = MerkelStuffs::from_list(vec![]).unwrap();

        let calc1 = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox, &empty_merkels).unwrap();
        let calc2 = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox, &empty_merkels).unwrap();
        assert_eq!(calc1.address, calc2.address, "same inputs should produce same address");
        assert_eq!(calc1.payload20, calc2.payload20);
    }

    #[test]
    fn unlock_script_prove_different_lockbox_different_address() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let empty_merkels = MerkelStuffs::from_list(vec![]).unwrap();

        let lockbox1 = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        let lockbox2 = BytesW2::from(vec![Bytecode::PU8 as u8, 2, Bytecode::END as u8]).unwrap();

        let calc1 = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox1, &empty_merkels).unwrap();
        let calc2 = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox2, &empty_merkels).unwrap();
        assert_ne!(calc1.address, calc2.address, "different lockbox should produce different address");
    }

    #[test]
    fn unlock_script_prove_merkel_posi_invalid() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        // Create a merkle step with invalid posi (2)
        let bad_step = PosiHash {
            posi: Uint1::from(2),
            hash: Hash::default(),
        };
        let merkels = MerkelStuffs::from_list(vec![bad_step]).unwrap();

        let err = UnlockScriptProve::calc_scriptmh_from_lockbox(&libs, &lockbox, &merkels).unwrap_err();
        assert!(err.contains("posi") && err.contains("invalid"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // P2SH Tool integration tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn p2sh_tool_single_leaf_tree() {
let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 42, Bytecode::END as u8]).unwrap();

        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs, lockbox },
        ]).unwrap();

        assert_eq!(tree.leaves().len(), 1);
        assert!(tree.address().is_scriptmh());

        // Proof for single leaf should be empty path
        let proof = tree.proof_for_index(0).unwrap();
        assert_eq!(proof.length(), 0);
    }

    #[test]
    fn p2sh_tool_two_leaf_tree_proofs_match() {
let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lb1 = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        let lb2 = BytesW2::from(vec![Bytecode::PU8 as u8, 2, Bytecode::END as u8]).unwrap();

        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb1 },
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb2 },
        ]).unwrap();

        assert_eq!(tree.leaves().len(), 2);

        // Both proofs should derive the same tree address
        for idx in 0..2 {
            let proof = tree.proof_for_index(idx).unwrap();
            let spec = &tree.leaves()[idx].spec;
            let calc = UnlockScriptProve::calc_scriptmh_from_lockbox(
                &spec.adrlibs, &spec.lockbox, &proof,
            ).unwrap();
            assert_eq!(calc.address, tree.address(), "proof for leaf {idx} should derive tree address");
        }
    }

    #[test]
    fn p2sh_tool_build_unlock_script_prove() {
let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lb1 = BytesW2::from(vec![Bytecode::PU8 as u8, 10, Bytecode::END as u8]).unwrap();
        let lb2 = BytesW2::from(vec![Bytecode::PU8 as u8, 20, Bytecode::END as u8]).unwrap();

        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb1 },
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb2 },
        ]).unwrap();

        let witness = BytesW2::from(vec![0xAA, 0xBB]).unwrap();
        let (addr, act, _calc) = tree.build_unlock_script_prove_unchecked(0, witness).unwrap();
        assert_eq!(addr, tree.address());
        assert!(addr.is_scriptmh());
        // The action should have correct lockbox from leaf 0
        assert_eq!(act.lockbox, tree.leaves()[0].spec.lockbox);
    }

    #[test]
    fn p2sh_tool_rejects_empty_specs() {
let err = P2shTool::build_canonical_tree(vec![]).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn p2sh_tool_rejects_duplicate_leaves() {
let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lb = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();

        let err = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb.clone() },
            P2shLeafSpec { adrlibs: libs.clone(), lockbox: lb },
        ]).unwrap_err();
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn p2sh_tool_build_unlock_script_prove_checked_success() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 7, Bytecode::END as u8]).unwrap();
        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs, lockbox },
        ]).unwrap();
        let witness = BytesW2::from(vec![0x11, 0x22, 0x33]).unwrap();

        let (addr, act, calc) = tree.build_unlock_script_prove_checked(1, 0, witness).unwrap();
        assert_eq!(addr, tree.address());
        assert_eq!(calc.address, tree.address());
        assert_eq!(act.merkels.length(), 0);
    }

    #[test]
    fn p2sh_tool_checked_rejects_invalid_lockbox() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        // 0x01 is an invalid bytecode in current ISA metadata table
        let invalid_lockbox = BytesW2::from(vec![0x01]).unwrap();
        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs, lockbox: invalid_lockbox },
        ]).unwrap();
        let witness = BytesW2::from(vec![]).unwrap();

        let err = tree.build_unlock_script_prove_checked(1, 0, witness).unwrap_err();
        assert!(err.contains("InstInvalid") || err.contains("invalid bytecode"), "{err}");
    }

    #[test]
    fn p2sh_tool_checked_rejects_oversized_witness() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs, lockbox },
        ]).unwrap();
        let over = SpaceCap::new(1).max_value_size + 1;
        let witness = BytesW2::from(vec![0u8; over]).unwrap();

        let err = tree.build_unlock_script_prove_checked(1, 0, witness).unwrap_err();
        assert!(err.contains("witness bytes too long"), "{err}");
    }

    #[test]
    fn p2sh_tool_checked_rejects_index_overflow() {
        let libs = ContractAddressW1::from_list(vec![]).unwrap();
        let lockbox = BytesW2::from(vec![Bytecode::PU8 as u8, 1, Bytecode::END as u8]).unwrap();
        let tree = P2shTool::build_canonical_tree(vec![
            P2shLeafSpec { adrlibs: libs, lockbox },
        ]).unwrap();
        let witness = BytesW2::from(vec![]).unwrap();

        let err = tree.build_unlock_script_prove_checked(1, 2, witness).unwrap_err();
        assert!(err.contains("leaf index") && err.contains("overflow"), "{err}");
    }

    // ═══════════════════════════════════════════════════
    // Cross-action integration tests
    // ═══════════════════════════════════════════════════

    #[test]
    fn deploy_then_call_contract() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        // Deploy a contract
        let sto = make_public_contract("greet", "return 0");
        execute_deploy(&mut ctx, 1, sto).unwrap();

        // Verify it exists
        let caddr = contract_addr(&main, 1);
        assert!(VMState::wrap(StateOperat::state(&mut ctx)).contract(&caddr).is_some());

        // Call it via sandbox
        let (gas, ret_json) = machine::sandbox_call(
            &mut ctx, caddr, "greet".to_owned(), "",
        ).unwrap();
        assert!(gas > 0);
        let ret: serde_json::Value = serde_json::from_str(&ret_json).unwrap();
        assert_eq!(ret.as_u64(), Some(0));
    }

    #[test]
    fn deploy_then_sandbox_call_missing_function_errors() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        let sto = make_public_contract("f", "return 0");
        execute_deploy(&mut ctx, 1, sto).unwrap();

        let caddr = contract_addr(&main, 1);
        let err = machine::sandbox_call(
            &mut ctx, caddr, "nonexistent".to_owned(), "",
        ).unwrap_err();
        assert!(err.contains("CallNotExist"), "{err}");
    }

    #[test]
    fn env_actions_at_different_heights() {
        let _guard = test_guard();
        let main = main_addr();

        for height in [0u64, 1, 100, 999999, u64::MAX / 2] {
            let tx = make_tx(3, main, vec![], 17);
            let mut ctx = make_ctx(height, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
            ctx.env.chain.fast_sync = true;

            let act = EnvHeight::new();
            let (_, res) = act.execute(&mut ctx).unwrap();
            assert_eq!(res, height.to_be_bytes().to_vec(), "height mismatch at {height}");
        }
    }

    #[test]
    fn multiple_deploys_different_nonces() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;
        fund_main_addr(&mut ctx);

        for nonce in 1..=5u32 {
            let sto = make_public_contract("f", "return 0");
            execute_deploy(&mut ctx, nonce, sto).unwrap();
            let caddr = contract_addr(&main, nonce);
            assert!(VMState::wrap(StateOperat::state(&mut ctx)).contract(&caddr).is_some(),
                "contract with nonce {nonce} should exist");
        }
    }

    #[test]
    fn deploy_rejects_negative_protocol_fee() {
        let _guard = test_guard();
        let main = main_addr();
        let tx = make_tx(3, main, vec![], 17);
        let mut ctx = make_ctx(1, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let sto = make_public_contract("f", "return 0");
        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(1u32);
        // Create a negative amount: 0 - 1 mei
        let neg_amt = Amount::zero().sub(&Amount::mei(1), field::AmtMode::BIGINT).unwrap();
        assert!(neg_amt.is_negative());
        act.protocol_cost = neg_amt;
        act.contract = sto;
        let err = act.execute(&mut ctx).unwrap_err();
        assert!(err.contains("negative") || err.contains("fee"), "{err}");
    }
}
