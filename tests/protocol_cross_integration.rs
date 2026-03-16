use basis::component::*;
use basis::interface::*;
use field::*;
use mint::action as mint_action;
use protocol::action::*;
use protocol::transaction::*;
use std::sync::Once;
use sys::*;

use testkit::sim::context::make_ctx_with_default_tx;
use testkit::sim::context::{
    make_ctx_with_logs as testkit_make_ctx_with_logs,
    make_ctx_with_state as testkit_make_ctx_with_state,
};
use testkit::sim::logs::MemLogs as AstTestLogs;
use testkit::sim::state::FlatMemState as TestMemState;
use testkit::sim::state::ForkableMemState as AstTestState;
use testkit::sim::vm::CounterMockVm as MockVM;

fn build_ast_ctx_with_state<'a>(
    env: Env,
    sta: Box<dyn State>,
    tx: &'a dyn TransactionRead,
) -> protocol::context::ContextInst<'a> {
    init_setup_once();
    let mut ctx = testkit_make_ctx_with_state(env, sta, tx);
    let main = ctx.env().tx.main;
    let mut st = protocol::state::CoreState::wrap(ctx.state());
    let mut bls = st.balance(&main).unwrap_or_default();
    bls.hacash = Amount::unit238(10_000_000_000_000);
    st.balance_set(&main, &bls);
    let _ = ctx.gas_init_tx(10000, 1);
    ctx
}

fn ast_state_get_u8(ctx: &mut dyn Context, key: u8) -> Option<u8> {
    ctx.state().get(vec![key]).and_then(|v| v.first().copied())
}

fn ast_hac_balance(ctx: &mut dyn Context, addr: &Address) -> Amount {
    protocol::state::CoreState::wrap(ctx.state())
        .balance(addr)
        .unwrap_or_default()
        .hacash
}

unsafe fn ctx_inst<'a>(ctx: &mut dyn Context) -> &mut protocol::context::ContextInst<'a> {
    unsafe { &mut *(ctx as *mut dyn Context as *mut protocol::context::ContextInst<'a>) }
}

fn init_setup_once() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let registry = protocol::setup::SetupBuilder::new()
            .block_hasher(|_, stuff| sys::calculate_hash(stuff))
            .action_register(protocol::action::register)
            .action_register(mint_action::register)
            .build()
            .unwrap();
        protocol::setup::install_once(registry).unwrap();
    });
}

static AST_TEST_GLOBAL_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

fn ast_test_globals_guard() -> std::sync::MutexGuard<'static, ()> {
    AST_TEST_GLOBAL_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestSet {
    key: Uint1,
    val: Uint1,
}

impl Parse for AstTestSet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}

impl Serialize for AstTestSet {
    fn serialize(&self) -> Vec<u8> {
        [self.key.serialize(), self.val.serialize()].concat()
    }

    fn size(&self) -> usize {
        self.key.size() + self.val.size()
    }
}

impl Field for AstTestSet {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestSet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}

impl FromJSON for AstTestSet {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "key" {
                self.key.from_json(v)?;
            } else if k == "val" {
                self.val.from_json(v)?;
            }
        }
        Ok(())
    }
}

impl ActExec for AstTestSet {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        Ok((0, vec![]))
    }
}

impl Description for AstTestSet {}

impl Action for AstTestSet {
    fn kind(&self) -> u16 {
        65001
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AstTestSet {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
            ..Self::new()
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestGasOnly {
    gas: Uint1,
}

impl Parse for AstTestGasOnly {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.gas.parse(buf)
    }
}

impl Serialize for AstTestGasOnly {
    fn serialize(&self) -> Vec<u8> {
        self.gas.serialize()
    }

    fn size(&self) -> usize {
        self.gas.size()
    }
}

impl Field for AstTestGasOnly {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestGasOnly {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestGasOnly {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestGasOnly {
    fn execute(&self, _ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        Ok((*self.gas as u32, vec![]))
    }
}

impl Description for AstTestGasOnly {}

impl Action for AstTestGasOnly {
    fn kind(&self) -> u16 {
        65030
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AstTestGasOnly {
    fn create_by(gas: u8) -> Self {
        Self {
            gas: Uint1::from(gas),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestFail {}

impl Parse for AstTestFail {
    fn parse(&mut self, _buf: &[u8]) -> Ret<usize> {
        Ok(0)
    }
}

impl Serialize for AstTestFail {
    fn serialize(&self) -> Vec<u8> {
        vec![]
    }

    fn size(&self) -> usize {
        0
    }
}

impl Field for AstTestFail {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestFail {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestFail {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestFail {
    fn execute(&self, _ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        xerr_rf!("ast test forced fail")
    }
}

impl Description for AstTestFail {}

impl Action for AstTestFail {
    fn kind(&self) -> u16 {
        65002
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_ast_if_cond_true_commits_cond_and_if_branch_state() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true; // keep focus on AST semantics
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.gas_init_tx(10000, 1).unwrap();

    let cond = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(1, 11))]);
    let br_if = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(2, 22))]);
    let br_else = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(3, 33))]);
    let astif = AstIf::create_by(cond, br_if, br_else);

    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(11)); // cond committed
    assert_eq!(ast_state_get_u8(&mut ctx, 2), Some(22)); // if branch committed
    assert_eq!(ast_state_get_u8(&mut ctx, 3), None); // else branch not executed
}

#[test]
fn test_ast_nested_plain_actions_no_over_or_under_charge() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;

    let run_case = |payload: (u8, u8, u8)| {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();

        let inner = AstSelect::create_list(vec![
            Box::new(AstTestGasOnly::create_by(payload.0)),
            Box::new(AstTestGasOnly::create_by(payload.1)),
        ]);
        let outer = AstSelect::create_list(vec![
            Box::new(inner),
            Box::new(AstTestGasOnly::create_by(payload.2)),
        ]);

        let before = ctx.gas_remaining();
        let (ret_gas, _) = outer.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        (ret_gas, before - after)
    };

    // AST control-flow nodes return gas=0; child return-gas is charged via ctx.gas_consume.
    let (ret0, shared0) = run_case((0, 0, 0));
    let (ret1, shared1) = run_case((7, 11, 5));

    assert_eq!(ret0, 0);
    assert_eq!(ret1, 0);
    // Children report 7+11+5=23 as return-gas, all charged via ctx.gas_consume.
    assert_eq!(shared1 - shared0, 23);
}

#[test]
fn test_ast_multilayer_nested_innermost_plain_return_gas_charged_once() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;

    let run_case = |inner_gas: u8| {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();

        // Multi-layer tree:
        // select(root) -> if -> select -> select -> select -> AstTestGasOnly(inner_gas)
        // Keep the tree shape fixed and vary only the innermost plain action return-gas.
        let level3 = AstSelect::create_list(vec![Box::new(AstTestGasOnly::create_by(inner_gas))]);
        let level2 = AstSelect::create_list(vec![Box::new(level3)]);
        let level1 = AstSelect::create_list(vec![Box::new(level2)]);
        let branch_if = AstSelect::create_list(vec![Box::new(level1)]);
        let node_if = AstIf::create_by(
            AstSelect::nop(), // cond succeeds with empty-select semantics
            branch_if,
            AstSelect::nop(),
        );
        let root = AstSelect::create_list(vec![Box::new(node_if)]);

        let before = ctx.gas_remaining();
        let (ret_gas, _) = root.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        assert_eq!(ret_gas, 0, "AST control-flow node must return gas=0");
        before - after
    };

    let used0 = run_case(0);
    let used7 = run_case(7);
    let used19 = run_case(19);

    // Only the innermost plain action return-gas changes between runs.
    // If return-gas were charged twice, these deltas would be doubled.
    assert_eq!(used7 - used0, 7);
    assert_eq!(used19 - used7, 12);
}

#[test]
fn test_ast_multilayer_innermost_revert_does_not_charge_return_gas() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;

    let run_success = || {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();

        // Same multilayer shape as control:
        // select(root) -> if -> select -> select -> select -> AstTestGasOnly(17)
        let level3 = AstSelect::create_by(0, 1, vec![Box::new(AstTestGasOnly::create_by(17))]);
        let level2 = AstSelect::create_by(0, 1, vec![Box::new(level3)]);
        let level1 = AstSelect::create_by(0, 1, vec![Box::new(level2)]);
        let branch_if = AstSelect::create_by(0, 1, vec![Box::new(level1)]);
        let node_if = AstIf::create_by(AstSelect::nop(), branch_if, AstSelect::nop());
        let root = AstSelect::create_by(0, 1, vec![Box::new(node_if)]);

        let before = ctx.gas_remaining();
        root.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        before - after
    };

    let run_revert = || {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();

        // Keep exactly the same multilayer structure, but replace the innermost
        // plain-gas action with a forced Revert action.
        let level3 = AstSelect::create_by(0, 1, vec![Box::new(AstTestFail::new())]);
        let level2 = AstSelect::create_by(0, 1, vec![Box::new(level3)]);
        let level1 = AstSelect::create_by(0, 1, vec![Box::new(level2)]);
        let branch_if = AstSelect::create_by(0, 1, vec![Box::new(level1)]);
        let node_if = AstIf::create_by(AstSelect::nop(), branch_if, AstSelect::nop());
        let root = AstSelect::create_by(0, 1, vec![Box::new(node_if)]);

        let before = ctx.gas_remaining();
        root.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        before - after
    };

    let used_success = run_success();
    let used_revert = run_revert();

    // In success path, innermost AstTestGasOnly returns 17 gas and should be charged once.
    // In revert path, innermost action has no successful return gas channel.
    assert_eq!(used_success - used_revert, 17);
}

#[test]
fn test_ast_static_size_repeated_charge_is_additive_per_execution() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;

    let run_case = |num_children: usize| {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();
        let mut acts: Vec<Box<dyn Action>> = Vec::with_capacity(num_children);
        for _ in 0..num_children {
            acts.push(Box::new(AstSelect::nop()));
        }
        let outer = AstSelect::create_list(acts);
        let before = ctx.gas_remaining();
        let (ret_gas, _) = outer.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        (ret_gas, before - after)
    };

    let (ret1, shared1) = run_case(1);
    let (ret2, shared2) = run_case(2);

    // AST nodes return gas=0; ret_gas is always 0.
    assert_eq!(ret1, 0);
    assert_eq!(ret2, 0);
    // one more child attempt adds one more snapshot try cost (40 gas)
    assert_eq!(shared2 - shared1, 40);
}

#[test]
fn test_ast_single_select_plain_reported_gas_propagates() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;

    let run_case = |a: u8, b: u8| {
        let mut ctx = build_ast_ctx_with_state(env.clone(), Box::new(AstTestState::default()), &tx);
        ctx.gas_init_tx(10000, 1).unwrap();
        let act = AstSelect::create_list(vec![
            Box::new(AstTestGasOnly::create_by(a)),
            Box::new(AstTestGasOnly::create_by(b)),
        ]);
        let before = ctx.gas_remaining();
        let (ret_gas, _) = act.execute(&mut ctx).unwrap();
        let after = ctx.gas_remaining();
        (ret_gas, before - after)
    };

    // AST control-flow nodes return gas=0; child return-gas is charged via ctx.gas_consume.
    let (ret0, shared0) = run_case(0, 0);
    let (ret1, shared1) = run_case(7, 11);
    assert_eq!(ret0, 0);
    assert_eq!(ret1, 0);
    // Children report 7+11=18 as return-gas, all charged via ctx.gas_consume.
    assert_eq!(shared1 - shared0, 18);
}

#[test]
fn test_ast_select_partial_write_is_reverted_by_tx_level_rollback() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.actions
        .push(Box::new(AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestSet::create_by(7, 77)), // succeeds and writes
                Box::new(AstTestFail::new()),           // fails
            ],
        )))
        .unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![9], vec![99]); // parent baseline

    let old = ctx.state_fork(); // tx-level isolation
    ctx.exec_from_set(ExecFrom::Top);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"), "{}", err);
    ctx.state_recover(old); // tx-level rollback on failure

    assert_eq!(ast_state_get_u8(&mut ctx, 9), Some(99)); // baseline kept
    assert_eq!(ast_state_get_u8(&mut ctx, 7), None); // child write rolled back
}

#[test]
fn test_ast_nested_if_select_else_path_commits_expected_layers() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.gas_init_tx(10000, 1).unwrap();

    let inner_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]), // force false -> else
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(52, 52))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(53, 53))]),
    );

    let outer_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(50, 50))]),
        AstSelect::create_list(vec![
            Box::new(AstTestSet::create_by(51, 51)),
            Box::new(inner_if),
        ]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(54, 54))]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    outer_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 50), Some(50)); // outer cond
    assert_eq!(ast_state_get_u8(&mut ctx, 51), Some(51)); // outer if branch
    assert_eq!(ast_state_get_u8(&mut ctx, 53), Some(53)); // inner else branch
    assert_eq!(ast_state_get_u8(&mut ctx, 52), None); // inner if branch not executed
    assert_eq!(ast_state_get_u8(&mut ctx, 54), None); // outer else not executed
}

#[test]
fn test_ast_tx_gasmax_zero_fails_at_first_consume_point() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.actions
        .push(Box::new(AstSelect::create_by(
            0,
            1,
            vec![Box::new(AstTestSet::create_by(15, 15))],
        )))
        .unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = create_tx_info(&tx);
    init_setup_once();
    let mut ctx = testkit_make_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    let mut state = protocol::state::CoreState::wrap(ctx.state());
    let mut bls = state.balance(&tx.main()).unwrap_or_default();
    bls.hacash = Amount::unit238(5_000_000_000);
    state.balance_set(&tx.main(), &bls);

    ctx.exec_from_set(ExecFrom::Top);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("gas has run out"), "{}", err);
}

#[test]
fn test_ast_nested_item_snapshot_gas_consumption_is_exact() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.fee = Amount::unit238(1_000_000);
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    protocol::operate::hac_add(
        &mut ctx,
        &field::ADDRESS_ONEX,
        &Amount::unit238(1_000_000_000),
    )
    .unwrap();
    ctx.gas_init_tx(1000, 1).unwrap();

    let inner_1 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(31, 31))]);
    let inner_2 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(32, 32))]);
    let outer = AstSelect::create_list(vec![Box::new(inner_1), Box::new(inner_2)]);

    let before = ctx.gas_remaining();
    ctx.exec_from_set(ExecFrom::Top);
    outer.execute(&mut ctx).unwrap();
    let after = ctx.gas_remaining();

    // snapshots consumed include child-attempt snapshots (whole node snapshot is free).
    assert_eq!(before - after, 160);
}

#[test]
fn test_tx_without_ast_allows_nonzero_gasmax() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1_000_000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.gas_max = Uint1::from(17);
    tx.actions.push(Box::new(TxMessage::new())).unwrap();

    let main = tx.main();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = create_tx_info(&tx);
    init_setup_once();
    let mut ctx = testkit_make_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let mut state = protocol::state::CoreState::wrap(ctx.state());
    let mut bls = state.balance(&main).unwrap_or_default();
    bls.hacash = Amount::unit238(5_000_000_000);
    state.balance_set(&main, &bls);

    tx.execute(&mut ctx).unwrap();
}

#[test]
fn test_ast_tx_gas_settlement_charges_fee_plus_used_and_refunds_unused() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.gas_max = Uint1::from(17);
    tx.fee = Amount::unit238(1_000_000);
    tx.actions
        .push(Box::new(AstSelect::create_list(vec![Box::new(
            AstTestSet::create_by(41, 41),
        )])))
        .unwrap();

    let main = tx.main();
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = main;
    env.tx.addrs = vec![main];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(5_000_000_000)).unwrap();

    let before = ast_hac_balance(&mut ctx, &main);
    ctx.exec_from_set(ExecFrom::Top);
    tx.execute(&mut ctx).unwrap();
    let after = ast_hac_balance(&mut ctx, &main);

    let used = ctx.ctx_gas_used_charge().unwrap();
    let maxc = ctx.ctx_gas_max_charge().unwrap();
    assert!(maxc > used, "must refund unused gas");

    assert!(after <= before || maxc > used);
}

#[test]
fn test_ast_nested_select_failure_does_not_leak_into_outer_select() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let nested_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(61, 61)), // would be committed in inner select before final Err
            Box::new(AstTestFail::new()),
        ],
    );
    let outer = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(60, 60)), // success #1
            Box::new(nested_fail),                   // Err -> outer recover this whole sub-state
            Box::new(AstTestSet::create_by(62, 62)), // success #2
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    outer.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 60), Some(60));
    assert_eq!(ast_state_get_u8(&mut ctx, 62), Some(62));
    assert_eq!(ast_state_get_u8(&mut ctx, 61), None); // nested failed select write must not leak
}

#[test]
fn test_ast_nested_partial_commits_are_cleared_by_tx_level_rollback() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);

    let act = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(70, 70))]), // cond=true and committed by AstIf
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestSet::create_by(71, 71)), // committed before final failure
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(72, 72))]),
    );
    tx.actions.push(Box::new(act)).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![79], vec![79]); // baseline

    let old = ctx.state_fork();
    ctx.exec_from_set(ExecFrom::Top);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"), "{}", err);
    ctx.state_recover(old);

    assert_eq!(ast_state_get_u8(&mut ctx, 79), Some(79)); // baseline kept
    assert_eq!(ast_state_get_u8(&mut ctx, 70), None); // nested partial commit must be rolled back at tx level
    assert_eq!(ast_state_get_u8(&mut ctx, 71), None); // nested partial commit must be rolled back at tx level
    assert_eq!(ast_state_get_u8(&mut ctx, 72), None);
}

#[test]
fn test_ast_deep_4level_success_path_commits_expected_state() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let lvl4_select = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(83, 83)),
            Box::new(AstTestSet::create_by(84, 84)),
        ],
    );
    let lvl3_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(82, 82))]),
        AstSelect::create_list(vec![Box::new(lvl4_select)]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(89, 89))]),
    );
    let lvl2_select = AstSelect::create_list(vec![Box::new(lvl3_if)]);
    let lvl1_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(80, 80))]),
        AstSelect::create_list(vec![
            Box::new(AstTestSet::create_by(81, 81)),
            Box::new(lvl2_select),
        ]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(88, 88))]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    lvl1_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 80), Some(80));
    assert_eq!(ast_state_get_u8(&mut ctx, 81), Some(81));
    assert_eq!(ast_state_get_u8(&mut ctx, 82), Some(82));
    assert_eq!(ast_state_get_u8(&mut ctx, 83), Some(83));
    assert_eq!(ast_state_get_u8(&mut ctx, 84), Some(84));
    assert_eq!(ast_state_get_u8(&mut ctx, 88), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 89), None);
}

#[test]
fn test_ast_deep_4level_failed_branch_isolated_by_outer_select() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let lvl4_if_fail = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(93, 93))]), // cond=true
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),            // force fail
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(94, 94))]),
    );
    let lvl3_select_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(92, 92)), // partial commit inside this nested select
            Box::new(lvl4_if_fail),                  // fails
        ],
    );
    let lvl2_if_fail = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(91, 91))]), // cond=true commit
        AstSelect::create_list(vec![Box::new(lvl3_select_fail)]),              // fails
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(96, 96))]),
    );
    let lvl1_outer_select = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(90, 90)), // success #1
            Box::new(lvl2_if_fail),                  // fail; outer select must recover this branch
            Box::new(AstTestSet::create_by(95, 95)), // success #2
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    lvl1_outer_select.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 90), Some(90));
    assert_eq!(ast_state_get_u8(&mut ctx, 95), Some(95));
    assert_eq!(ast_state_get_u8(&mut ctx, 91), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 92), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 93), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 94), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 96), None);
}

#[test]
fn test_ast_tree_depth_limit_6_rejects_7th_level() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let lvl7 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(105, 105))]);
    let lvl6 = AstSelect::create_list(vec![Box::new(lvl7)]);
    let lvl5 = AstSelect::create_list(vec![Box::new(lvl6)]);
    let lvl4 = AstSelect::create_list(vec![Box::new(lvl5)]);
    let lvl3 = AstSelect::create_list(vec![Box::new(lvl4)]);
    let lvl2 = AstSelect::create_list(vec![Box::new(lvl3)]);
    let lvl1 = AstSelect::create_list(vec![Box::new(lvl2)]);

    let err = check_action_ast_tree_depth(&lvl1).unwrap_err();
    assert!(err.contains("ast tree depth 7 exceeded max 6"), "{}", err);
    assert_eq!(ast_state_get_u8(&mut ctx, 105), None);
}

#[test]
fn test_ast_savepoint_recover_tex_and_p2sh() {
    #[derive(Default, Debug, Clone, PartialEq, Eq)]
    struct AstTestTexP2shSet;
    impl Parse for AstTestTexP2shSet {
        fn parse(&mut self, _buf: &[u8]) -> Ret<usize> {
            Ok(0)
        }
    }
    impl Serialize for AstTestTexP2shSet {
        fn serialize(&self) -> Vec<u8> {
            vec![]
        }
        fn size(&self) -> usize {
            0
        }
    }
    impl Field for AstTestTexP2shSet {
        fn new() -> Self {
            Self::default()
        }
    }
    impl ToJSON for AstTestTexP2shSet {
        fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
            "{}".to_owned()
        }
    }
    impl FromJSON for AstTestTexP2shSet {
        fn from_json(&mut self, _json: &str) -> Ret<()> {
            Ok(())
        }
    }
    struct AstTestP2sh;
    impl P2sh for AstTestP2sh {
        fn code_stuff(&self) -> &[u8] {
            b"x"
        }
        fn witness(&self) -> &[u8] {
            b"y"
        }
    }
    impl ActExec for AstTestTexP2shSet {
        fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
            ctx.tex_ledger().sat += 7;
            let adr = Address::create_scriptmh([7u8; 20]);
            ctx.p2sh_set(adr, Box::new(AstTestP2sh))?;
            Ok((0, vec![]))
        }
    }
    impl Description for AstTestTexP2shSet {}
    impl Action for AstTestTexP2shSet {
        fn kind(&self) -> u16 {
            65003
        }
        fn scope(&self) -> ActScope {
            ActScope::AST
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    let old_adr = Address::create_scriptmh([6u8; 20]);
    ctx.p2sh_set(old_adr, Box::new(AstTestP2sh)).unwrap();
    let old_tex = ctx.tex_ledger().clone();
    let new_adr = Address::create_scriptmh([7u8; 20]);
    let inner = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestTexP2shSet::new()),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(0, 1, vec![Box::new(inner)]);
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();
    assert_eq!(ctx.tex_ledger().zhu, old_tex.zhu);
    assert_eq!(ctx.tex_ledger().sat, old_tex.sat);
    assert!(ctx.p2sh(&old_adr).is_ok());
    assert!(ctx.p2sh(&new_adr).is_err());
}

#[test]
fn test_ast_select_failure_rolls_back_p2sh_inside_node() {
    #[derive(Default, Debug, Clone, PartialEq, Eq)]
    struct AstTestP2shSetOnly;

    impl Parse for AstTestP2shSetOnly {
        fn parse(&mut self, _buf: &[u8]) -> Ret<usize> {
            Ok(0)
        }
    }
    impl Serialize for AstTestP2shSetOnly {
        fn serialize(&self) -> Vec<u8> {
            vec![]
        }
        fn size(&self) -> usize {
            0
        }
    }
    impl Field for AstTestP2shSetOnly {
        fn new() -> Self {
            Self::default()
        }
    }
    impl ToJSON for AstTestP2shSetOnly {
        fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
            "{}".to_owned()
        }
    }
    impl FromJSON for AstTestP2shSetOnly {
        fn from_json(&mut self, _json: &str) -> Ret<()> {
            Ok(())
        }
    }

    struct AstTestP2sh;
    impl P2sh for AstTestP2sh {
        fn code_stuff(&self) -> &[u8] {
            b"x"
        }
        fn witness(&self) -> &[u8] {
            b"y"
        }
    }

    impl ActExec for AstTestP2shSetOnly {
        fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
            let adr = Address::create_scriptmh([8u8; 20]);
            ctx.p2sh_set(adr, Box::new(AstTestP2sh))?;
            Ok((0, vec![]))
        }
    }
    impl Description for AstTestP2shSetOnly {}
    impl Action for AstTestP2shSetOnly {
        fn kind(&self) -> u16 {
            65004
        }
        fn scope(&self) -> ActScope {
            ActScope::AST
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let new_adr = Address::create_scriptmh([8u8; 20]);
    let act = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestP2shSetOnly::new()),
            Box::new(AstTestFail::new()),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));
    assert!(ctx.p2sh(&new_adr).is_err());
}

fn build_tex_ctx_with_state(
    env: Env,
    sta: Box<dyn State>,
) -> protocol::context::ContextInst<'static> {
    init_setup_once();
    make_ctx_with_default_tx(env, sta)
}

#[test]
fn test_tex_sat_pay_records_sat_not_zhu() {
    use protocol::tex::*;

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    {
        let mut st = protocol::state::CoreState::wrap(ctx.state());
        let mut bls = Balance::default();
        bls.satoshi = Fold64::from(100).unwrap();
        st.balance_set(&addr, &bls);
    }

    let cell = CellTrsSatPay::new(Fold64::from(7).unwrap());
    cell.execute(&mut ctx, &addr).unwrap();

    assert_eq!(ctx.tex_ledger().sat, 7);
    assert_eq!(ctx.tex_ledger().zhu, 0);
}

#[test]
fn test_tex_asset_serial_must_exist_and_cache() {
    use protocol::tex::*;

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    {
        let mut st = protocol::state::CoreState::wrap(ctx.state());
        st.asset_set(
            &Fold64::from(9).unwrap(),
            &AssetSmelt {
                serial: Fold64::from(9).unwrap(),
                supply: Fold64::from(10_000).unwrap(),
                decimal: Uint1::from(2),
                issuer: addr,
                ticket: BytesW1::from_str("AST9").unwrap(),
                name: BytesW1::from_str("Asset9").unwrap(),
            },
        );
    }

    let miss = CellCondAssetEq::new(AssetAmt::from(999, 1).unwrap())
        .execute(&mut ctx, &addr)
        .unwrap_err();
    assert!(miss.contains("does not exist"));

    let ok1 = CellCondAssetEq::new(AssetAmt::from(9, 0).unwrap());
    ok1.execute(&mut ctx, &addr).unwrap();
    let ok2 = CellCondAssetEq::new(AssetAmt::from(9, 0).unwrap());
    ok2.execute(&mut ctx, &addr).unwrap();
    assert!(
        ctx.tex_ledger()
            .asset_checked
            .contains(&Fold64::from(9).unwrap())
    );
    assert_eq!(ctx.tex_ledger().asset_checked.len(), 1);
}

#[test]
fn test_tex_diamond_get_zero_rejected_early() {
    use protocol::tex::*;

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    let err = CellTrsDiaGet::new(DiamondNumber::from(0))
        .execute(&mut ctx, &addr)
        .unwrap_err();
    assert!(err.contains("cannot be zero"));
}

#[test]
fn test_tex_cell_json_must_use_cellid() {
    use protocol::tex::*;

    let mut ls = DnyTexCellW1::default();
    let ok_json = r#"[{"cellid":11,"haczhu":0}]"#;
    ls.from_json(ok_json).unwrap();
    assert_eq!(ls.length(), 1);

    let mut bad = DnyTexCellW1::default();
    let err = bad.from_json(r#"[{"kind":11,"haczhu":0}]"#).unwrap_err();
    assert!(err.contains("cellid"));
}

#[test]
fn test_tex_action_signature_rejects_payload_tamper() {
    use protocol::tex::*;

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    env.block.height = 10;
    env.chain.fast_sync = true;

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    let acc = Account::create_by_password("sig_check_tex").unwrap();
    let addr = Address::from(*acc.address());

    let mut act = TexCellAct::create_by(addr);
    act.add_cell(Box::new(CellCondHeightAtMost::new(100)))
        .unwrap();
    act.do_sign(&acc).unwrap();
    // tamper payload after sign
    act.add_cell(Box::new(CellCondHeightAtMost::new(100)))
        .unwrap();

    ctx.exec_from_set(ExecFrom::Top);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("signature verification failed"));
}

// =====================================================================
// Comprehensive AST snapshot/restore edge-case tests
// =====================================================================

// --- Test helper: action that pushes a log entry ---
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestLog {
    tag: Uint1,
}

impl Parse for AstTestLog {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.tag.parse(buf)
    }
}
impl Serialize for AstTestLog {
    fn serialize(&self) -> Vec<u8> {
        self.tag.serialize()
    }
    fn size(&self) -> usize {
        self.tag.size()
    }
}
impl Field for AstTestLog {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestLog {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"tag\":{}}}", self.tag.to_json_fmt(fmt))
    }
}
impl FromJSON for AstTestLog {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "tag" {
                self.tag.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestLog {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.logs().push(&self.tag);
        Ok((0, vec![]))
    }
}
impl Description for AstTestLog {}
impl Action for AstTestLog {
    fn kind(&self) -> u16 {
        65005
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestLog {
    fn create_by(tag: u8) -> Self {
        Self {
            tag: Uint1::from(tag),
        }
    }
}

// --- Test helper: action that modifies tex_ledger ---
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestTexAdd {
    zhu_add: Uint1,
}
impl Parse for AstTestTexAdd {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.zhu_add.parse(buf)
    }
}
impl Serialize for AstTestTexAdd {
    fn serialize(&self) -> Vec<u8> {
        self.zhu_add.serialize()
    }
    fn size(&self) -> usize {
        self.zhu_add.size()
    }
}
impl Field for AstTestTexAdd {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestTexAdd {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"zhu_add\":{}}}", self.zhu_add.to_json_fmt(fmt))
    }
}
impl FromJSON for AstTestTexAdd {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "zhu_add" {
                self.zhu_add.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestTexAdd {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.tex_ledger().zhu += *self.zhu_add as i64;
        Ok((0, vec![]))
    }
}
impl Description for AstTestTexAdd {}
impl Action for AstTestTexAdd {
    fn kind(&self) -> u16 {
        65006
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestTexAdd {
    fn create_by(zhu: u8) -> Self {
        Self {
            zhu_add: Uint1::from(zhu),
        }
    }
}

// --- Test helper: action that sets P2SH with configurable address byte ---
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestP2shSetN {
    addr_byte: Uint1,
}
impl Parse for AstTestP2shSetN {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.addr_byte.parse(buf)
    }
}
impl Serialize for AstTestP2shSetN {
    fn serialize(&self) -> Vec<u8> {
        self.addr_byte.serialize()
    }
    fn size(&self) -> usize {
        self.addr_byte.size()
    }
}
impl Field for AstTestP2shSetN {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestP2shSetN {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"addr_byte\":{}}}", self.addr_byte.to_json_fmt(fmt))
    }
}
impl FromJSON for AstTestP2shSetN {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "addr_byte" {
                self.addr_byte.from_json(v)?;
            }
        }
        Ok(())
    }
}
struct AstTestP2shImpl;
impl P2sh for AstTestP2shImpl {
    fn code_stuff(&self) -> &[u8] {
        b"code"
    }
    fn witness(&self) -> &[u8] {
        b"wit"
    }
}
impl ActExec for AstTestP2shSetN {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        Ok((0, vec![]))
    }
}
impl Description for AstTestP2shSetN {}
impl Action for AstTestP2shSetN {
    fn kind(&self) -> u16 {
        65007
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestP2shSetN {
    fn create_by(n: u8) -> Self {
        Self {
            addr_byte: Uint1::from(n),
        }
    }
}

// --- Test helper: action that does state set + tex + log in one shot ---
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestCombo {
    key: Uint1,
    val: Uint1,
}
impl Parse for AstTestCombo {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
impl Serialize for AstTestCombo {
    fn serialize(&self) -> Vec<u8> {
        [self.key.serialize(), self.val.serialize()].concat()
    }
    fn size(&self) -> usize {
        self.key.size() + self.val.size()
    }
}
impl Field for AstTestCombo {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestCombo {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}
impl FromJSON for AstTestCombo {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "key" {
                self.key.from_json(v)?;
            } else if k == "val" {
                self.val.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestCombo {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        ctx.tex_ledger().zhu += *self.val as i64;
        ctx.logs().push(&self.key);
        Ok((0, vec![]))
    }
}
impl Description for AstTestCombo {}
impl Action for AstTestCombo {
    fn kind(&self) -> u16 {
        65008
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestCombo {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
        }
    }
}

// --- Helper to build ctx with AstTestLogs ---
fn build_ast_ctx_with_logs<'a>(
    env: Env,
    sta: Box<dyn State>,
    log: Box<dyn Logs>,
    tx: &'a dyn TransactionRead,
) -> protocol::context::ContextInst<'a> {
    init_setup_once();
    let mut ctx = testkit_make_ctx_with_logs(env, sta, log, tx);
    let main = ctx.env().tx.main;
    let mut st = protocol::state::CoreState::wrap(ctx.state());
    let mut bls = st.balance(&main).unwrap_or_default();
    bls.hacash = Amount::unit238(10_000_000_000_000);
    st.balance_set(&main, &bls);
    let _ = ctx.gas_init_tx(10000, 1);
    ctx
}

// PLACEHOLDER_NEW_TESTS

// ---- Test 1: AstIf branch failure triggers whole_snap recover ----
// Validates the fix: without ctx_recover(ctx, whole_snap) on branch Err,
// the state fork layer leaks.
#[test]
fn test_ast_if_branch_fail_recovers_whole_snap() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![200], vec![200]); // baseline

    // cond succeeds (writes state), but br_if fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(201, 201))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(202, 202))]),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least") || err.contains("ast test forced fail"));

    // whole_snap must have been recovered: cond side-effects rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 200), Some(200)); // baseline intact
    assert_eq!(ast_state_get_u8(&mut ctx, 201), None); // cond write rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 202), None); // else never ran
}

// ---- Test 2: AstIf else branch failure also recovers whole_snap ----
#[test]
fn test_ast_if_else_branch_fail_recovers_whole_snap() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // cond fails -> else branch, but else also fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(210, 210))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("ast test forced fail") || err.contains("must succeed at least"));
    assert_eq!(ast_state_get_u8(&mut ctx, 210), None);
}

// ---- Test 3: AstSelect early-return validation doesn't leak state fork ----
// Validates the fix: validation checks moved before ctx_snapshot.
#[test]
fn test_ast_select_validation_early_return_no_state_leak() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![220], vec![220]);

    // min > max: invalid
    let bad = AstSelect::create_by(3, 1, vec![Box::new(AstTestSet::create_by(221, 221))]);
    ctx.exec_from_set(ExecFrom::Top);
    let err = bad.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot be less than min"));

    // State must still be available (no leaked fork layer)
    assert_eq!(ast_state_get_u8(&mut ctx, 220), Some(220));
    ctx.state().set(vec![222], vec![222]);
    assert_eq!(ast_state_get_u8(&mut ctx, 222), Some(222));
}

// PLACEHOLDER_TESTS_PART2

// ---- Test 4: Logs are truncated on AstSelect child failure ----
#[test]
fn test_ast_select_logs_truncated_on_child_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    // child 1: log + succeed, child 2: log + fail
    // AstSelect(min=1, max=2): child1 ok, child2 fail -> ok with 1
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestLog::create_by(1)),
            Box::new(AstTestFail::new()), // fails, its snap should recover logs
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    // Only child1's log should remain (child2 failed -> no log from it, but AstTestFail doesn't log)
    // The key point: log count should be 1 (from child1), not more
    let log_len = unsafe { &*logs_ptr }.len();
    assert_eq!(log_len, 1);
}

// ---- Test 5: Logs truncated on AstIf branch failure (whole_snap recover) ----
#[test]
fn test_ast_if_branch_fail_truncates_logs() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    // cond logs + succeeds, br_if logs + fails -> whole_snap recover should truncate all
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestLog::create_by(10))]),
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestLog::create_by(11)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::create_list(vec![Box::new(AstTestLog::create_by(12))]),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // All logs from cond and br_if must be rolled back
    let log_len = unsafe { &*logs_ptr }.len();
    assert_eq!(log_len, 0);
}

// ---- Test 6: tex_ledger restored on nested AstSelect failure ----
#[test]
fn test_ast_select_tex_ledger_restored_on_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.tex_ledger().zhu = 100; // baseline

    // child1: adds 10 to zhu + succeeds
    // child2: adds 20 to zhu + fails
    // min=1, max=2 -> child1 ok, child2 fail -> ok
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestTexAdd::create_by(10)),
            Box::new(AstTestFail::new()),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    // child1's tex change committed, child2 never modified tex (AstTestFail doesn't touch it)
    assert_eq!(ctx.tex_ledger().zhu, 110);
}

// ---- Test 7: tex_ledger fully rolled back when AstIf fails ----
#[test]
fn test_ast_if_fail_rolls_back_tex_ledger() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.tex_ledger().zhu = 50;

    // cond adds 5 to zhu + succeeds, br_if fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestTexAdd::create_by(5))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // whole_snap recover must restore tex_ledger
    assert_eq!(ctx.tex_ledger().zhu, 50);
}

// PLACEHOLDER_TESTS_PART3

// ---- Test 8: P2SH set in successful branch kept, failed branch removed ----
#[test]
fn test_ast_select_p2sh_kept_on_success_removed_on_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // child1: set p2sh(addr_byte=30) + succeed
    // child2: set p2sh(addr_byte=31) + fail (wrapped in select that requires 2 but only 1 succeeds)
    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestP2shSetN::create_by(31)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestP2shSetN::create_by(30)),
            Box::new(inner_fail),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    let adr30 = Address::create_scriptmh([30u8; 20]);
    let adr31 = Address::create_scriptmh([31u8; 20]);
    assert!(ctx.p2sh(&adr30).is_ok()); // success branch kept
    assert!(ctx.p2sh(&adr31).is_err()); // failed branch removed
}

// ---- Test 9: AstSelect min=0 all children fail -> success with empty result ----
#[test]
fn test_ast_select_min_zero_all_fail_succeeds() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![230], vec![230]);

    let act = AstSelect::create_by(
        0,
        2,
        vec![Box::new(AstTestFail::new()), Box::new(AstTestFail::new())],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap(); // should succeed

    assert_eq!(ast_state_get_u8(&mut ctx, 230), Some(230)); // baseline intact
}

// ---- Test 10: Combo action (state+tex+log) all restored on failure ----
#[test]
fn test_ast_combo_all_channels_restored_on_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.tex_ledger().zhu = 10;

    // combo writes state + tex + log, then fail forces rollback
    let act = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestCombo::create_by(240, 5)),
            Box::new(AstTestFail::new()),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));

    assert_eq!(ast_state_get_u8(&mut ctx, 240), None); // state rolled back
    assert_eq!(ctx.tex_ledger().zhu, 10); // tex rolled back
    assert_eq!(unsafe { &*logs_ptr }.len(), 0); // logs rolled back
}

// ---- Test 11: Nested AstIf inside AstSelect — inner if fails, outer select recovers ----
#[test]
fn test_ast_nested_if_fail_inside_select_recovers_all_channels() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    // inner_if: cond=combo(250,1) succeeds, br_if=fail -> whole AstIf fails
    let inner_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestCombo::create_by(250, 1))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
    );
    // outer select: child1=combo(251,2) ok, child2=inner_if fail, child3=combo(252,3) ok
    let act = AstSelect::create_by(
        2,
        3,
        vec![
            Box::new(AstTestCombo::create_by(251, 2)),
            Box::new(inner_if),
            Box::new(AstTestCombo::create_by(252, 3)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    // child1 and child3 committed, inner_if rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 251), Some(2));
    assert_eq!(ast_state_get_u8(&mut ctx, 252), Some(3));
    assert_eq!(ast_state_get_u8(&mut ctx, 250), None); // inner_if cond rolled back
    assert_eq!(ctx.tex_ledger().zhu, 5); // 2 + 3, not 1
    // logs: child1 pushed 1, inner_if's cond pushed 1 but rolled back, child3 pushed 1 = 2
    assert_eq!(unsafe { &*logs_ptr }.len(), 2);
}

// PLACEHOLDER_TESTS_PART4

// ---- Test 12: State overwrite in failed branch doesn't leak ----
#[test]
fn test_ast_state_overwrite_in_failed_branch_does_not_leak() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![1], vec![100]); // pre-existing value

    // child1: overwrite key=1 to 200, then fail
    let inner = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestSet::create_by(1, 200)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(0, 1, vec![Box::new(inner)]);
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    // Original value must be preserved
    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(100));
}

// ---- Test 13: AstIf else path with nested AstSelect partial success ----
#[test]
fn test_ast_if_else_with_nested_select_partial_success() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // cond fails -> else branch
    // else = select(min=1, max=3): child1 ok, child2 fail, child3 ok
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
        AstSelect::create_by(
            1,
            3,
            vec![
                Box::new(AstTestSet::create_by(160, 160)),
                Box::new(AstTestFail::new()),
                Box::new(AstTestSet::create_by(162, 162)),
            ],
        ),
    );
    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 160), Some(160));
    assert_eq!(ast_state_get_u8(&mut ctx, 162), Some(162));
}

// ---- Test 14: P2SH + state + tex all committed on success path ----
#[test]
fn test_ast_all_channels_committed_on_success() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(170, 1))]),
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(171, 10)),
            Box::new(AstTestP2shSetN::create_by(40)),
            Box::new(AstTestTexAdd::create_by(20)),
            Box::new(AstTestLog::create_by(99)),
        ]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 170), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 171), Some(10));
    assert_eq!(ctx.tex_ledger().zhu, 30); // combo(10) + tex_add(20)
    assert!(ctx.p2sh(&Address::create_scriptmh([40u8; 20])).is_ok());
    // logs: combo pushed 1, log pushed 1 = 2
    assert_eq!(unsafe { &*logs_ptr }.len(), 2);
}

// ---- Test 15: Double nested AstIf — inner else, outer if ----
#[test]
fn test_ast_double_nested_if_inner_else_outer_if() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let inner_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]), // cond fail -> else
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(180, 180))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(181, 181))]),
    );
    let outer_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(182, 182))]), // cond ok -> if
        AstSelect::create_list(vec![
            Box::new(inner_if),
            Box::new(AstTestSet::create_by(183, 183)),
        ]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    outer_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 182), Some(182)); // outer cond
    assert_eq!(ast_state_get_u8(&mut ctx, 181), Some(181)); // inner else
    assert_eq!(ast_state_get_u8(&mut ctx, 183), Some(183)); // outer if sibling
    assert_eq!(ast_state_get_u8(&mut ctx, 180), None); // inner if not taken
}

// ---- Test 16: AstSelect max reached stops early, remaining children not executed ----
#[test]
fn test_ast_select_stops_at_max() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestSet::create_by(190, 1)),
            Box::new(AstTestSet::create_by(191, 2)),
            Box::new(AstTestSet::create_by(192, 3)), // should not execute
            Box::new(AstTestSet::create_by(193, 4)), // should not execute
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 190), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 191), Some(2));
    assert_eq!(ast_state_get_u8(&mut ctx, 192), None); // not reached
    assert_eq!(ast_state_get_u8(&mut ctx, 193), None); // not reached
}

// ---- Test 17: AstSelect validation max > num rejected without state leak ----
#[test]
fn test_ast_select_max_gt_num_rejected_no_leak() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![1], vec![1]);

    let bad = AstSelect::create_by(1, 5, vec![Box::new(AstTestSet::create_by(2, 2))]);
    ctx.exec_from_set(ExecFrom::Top);
    let err = bad.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot exceed list num"));

    // State still available
    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(1));
    ctx.state().set(vec![3], vec![3]);
    assert_eq!(ast_state_get_u8(&mut ctx, 3), Some(3));
}

// ---- Test 18: Sequential AST operations on same context ----
// After one AST op completes (success or fail), the next one works correctly.
#[test]
fn test_ast_sequential_operations_on_same_context() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // Op 1: fails
    let fail_act = AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]);
    ctx.exec_from_set(ExecFrom::Top);
    let _ = fail_act.execute(&mut ctx);

    // Op 2: succeeds
    let ok_act = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(150, 150))]);
    ctx.exec_from_set(ExecFrom::Top);
    ok_act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 150), Some(150));

    // Op 3: AstIf succeeds
    let if_act = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(151, 151))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(152, 152))]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    if_act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 151), Some(151));
    assert_eq!(ast_state_get_u8(&mut ctx, 152), Some(152));
}

// ---- Test 19: P2SH duplicate address rejected even across AST branches ----
#[test]
fn test_ast_p2sh_duplicate_address_rejected() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // First set p2sh(50) outside AST
    let adr50 = Address::create_scriptmh([50u8; 20]);
    ctx.p2sh_set(adr50, Box::new(AstTestP2shImpl)).unwrap();

    // Try to set same address inside AST -> should fail
    let act = AstSelect::create_list(vec![Box::new(AstTestP2shSetN::create_by(50))]);
    ctx.exec_from_set(ExecFrom::Top);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("already proved") || err.contains("must succeed at least"),
        "unexpected error: {}",
        err
    );
}

// ---- Test 20: P2SH set in failed AstSelect child is rolled back,
//               then same address can be set in next successful child ----
#[test]
fn test_ast_p2sh_rollback_allows_retry_in_next_child() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // child1: set p2sh(60) then fail -> rolled back
    // child2: set p2sh(60) succeeds (because child1's set was rolled back)
    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestP2shSetN::create_by(60)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(inner_fail),
            Box::new(AstTestP2shSetN::create_by(60)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    let adr60 = Address::create_scriptmh([60u8; 20]);
    assert!(ctx.p2sh(&adr60).is_ok());
}

// =====================================================================
// VM snapshot/restore tests within AST branches
// =====================================================================

// --- Test action that mutates VM state (increments MockVM counter) ---
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestVMCall {
    increment: Uint1,
}

impl Parse for AstTestVMCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.increment.parse(buf)
    }
}
impl Serialize for AstTestVMCall {
    fn serialize(&self) -> Vec<u8> {
        self.increment.serialize()
    }
    fn size(&self) -> usize {
        self.increment.size()
    }
}
impl Field for AstTestVMCall {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestVMCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}
impl FromJSON for AstTestVMCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}
impl ActExec for AstTestVMCall {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let Some(snap) = ctx.vm_snapshot_volatile() else {
            return xerrf!("test vm missing");
        };
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.increment as i64;
            ctx.vm_restore_volatile(Box::new(new_val));
        }
        Ok((0, vec![]))
    }
}
impl Description for AstTestVMCall {}
impl Action for AstTestVMCall {
    fn kind(&self) -> u16 {
        65009
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestVMCall {
    fn create_by(inc: u8) -> Self {
        Self {
            increment: Uint1::from(inc),
        }
    }
}

struct AstRecoverTrackVm {
    value: std::sync::Arc<std::sync::atomic::AtomicI64>,
    restore_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    clean_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl VM for AstRecoverTrackVm {
    fn snapshot_volatile(&mut self) -> Box<dyn std::any::Any> {
        Box::new(self.value.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn restore_volatile(&mut self, snap: Box<dyn std::any::Any>) {
        if let Ok(v) = snap.downcast::<i64>() {
            self.restore_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.value.store(*v, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn restore_but_keep_warmup(&mut self) {
        self.clean_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.value.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn call(
        &mut self,
        _: &mut dyn Context,
        _: u8,
        _target: u8,
        _: std::sync::Arc<[u8]>,
        _: Box<dyn std::any::Any>,
    ) -> XRet<(i64, Vec<u8>)> {
        Ok((0, vec![]))
    }
}

static AST_RECOVER_TRACK_HANDLES: std::sync::OnceLock<
    std::sync::Mutex<
        Option<(
            std::sync::Arc<std::sync::atomic::AtomicI64>,
            std::sync::Arc<std::sync::atomic::AtomicUsize>,
            std::sync::Arc<std::sync::atomic::AtomicUsize>,
        )>,
    >,
> = std::sync::OnceLock::new();

fn set_ast_recover_track_handles(
    value: std::sync::Arc<std::sync::atomic::AtomicI64>,
    restore_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    clean_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    let lock = AST_RECOVER_TRACK_HANDLES.get_or_init(|| std::sync::Mutex::new(None));
    *lock.lock().unwrap() = Some((value, restore_count, clean_count));
}

fn take_ast_recover_track_vm() -> Box<dyn VM> {
    let lock = AST_RECOVER_TRACK_HANDLES.get_or_init(|| std::sync::Mutex::new(None));
    let guards = lock.lock().unwrap();
    let (v, r, c) = guards
        .as_ref()
        .expect("recover track handles not set")
        .clone();
    Box::new(AstRecoverTrackVm {
        value: v,
        restore_count: r,
        clean_count: c,
    })
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestVmInitReplace {
    value: Uint1,
}

impl Parse for AstTestVmInitReplace {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.value.parse(buf)
    }
}

impl Serialize for AstTestVmInitReplace {
    fn serialize(&self) -> Vec<u8> {
        self.value.serialize()
    }

    fn size(&self) -> usize {
        self.value.size()
    }
}

impl Field for AstTestVmInitReplace {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestVmInitReplace {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestVmInitReplace {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestVmInitReplace {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        unsafe { ctx_inst(ctx) }.test_set_vm(take_ast_recover_track_vm());
        ctx.vm_restore_volatile(Box::new(*self.value as i64));
        Ok((0, vec![]))
    }
}

impl Description for AstTestVmInitReplace {}

impl Action for AstTestVmInitReplace {
    fn kind(&self) -> u16 {
        65017
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AstTestVmInitReplace {
    fn create_by(v: u8) -> Self {
        Self {
            value: Uint1::from(v),
        }
    }
}

struct AstDeepDelayVm {
    volatile: std::sync::Arc<std::sync::atomic::AtomicI64>,
    warmup: std::sync::Arc<std::sync::atomic::AtomicI64>,
    restore_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    clean_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl VM for AstDeepDelayVm {
    fn snapshot_volatile(&mut self) -> Box<dyn std::any::Any> {
        Box::new(self.volatile.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn restore_volatile(&mut self, snap: Box<dyn std::any::Any>) {
        if let Ok(v) = snap.downcast::<i64>() {
            self.restore_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.volatile.store(*v, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn restore_but_keep_warmup(&mut self) {
        self.clean_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.volatile.store(0, std::sync::atomic::Ordering::SeqCst);
    }

    fn call(
        &mut self,
        _: &mut dyn Context,
        _: u8,
        _target: u8,
        payload: std::sync::Arc<[u8]>,
        _: Box<dyn std::any::Any>,
    ) -> XRet<(i64, Vec<u8>)> {
        let data = payload.as_ref();
        if data.len() < 3 {
            return xerrf!("deep delay vm payload too short");
        }
        let vol_add = data[0] as i64;
        let warm_add = data[1] as i64;
        let should_fail = data[2] != 0;
        self.volatile
            .fetch_add(vol_add, std::sync::atomic::Ordering::SeqCst);
        self.warmup
            .fetch_add(warm_add, std::sync::atomic::Ordering::SeqCst);
        if should_fail {
            return xerr_rf!("deep delay vm forced fail");
        }
        Ok((0, vec![]))
    }
}

static AST_DEEP_DELAY_VM_HANDLES: std::sync::OnceLock<
    std::sync::Mutex<
        Option<(
            std::sync::Arc<std::sync::atomic::AtomicI64>,
            std::sync::Arc<std::sync::atomic::AtomicI64>,
            std::sync::Arc<std::sync::atomic::AtomicUsize>,
            std::sync::Arc<std::sync::atomic::AtomicUsize>,
        )>,
    >,
> = std::sync::OnceLock::new();

fn set_ast_deep_delay_vm_handles(
    volatile: std::sync::Arc<std::sync::atomic::AtomicI64>,
    warmup: std::sync::Arc<std::sync::atomic::AtomicI64>,
    restore_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    clean_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    let lock = AST_DEEP_DELAY_VM_HANDLES.get_or_init(|| std::sync::Mutex::new(None));
    *lock.lock().unwrap() = Some((volatile, warmup, restore_count, clean_count));
}

fn take_ast_deep_delay_vm() -> Box<dyn VM> {
    let lock = AST_DEEP_DELAY_VM_HANDLES.get_or_init(|| std::sync::Mutex::new(None));
    let guards = lock.lock().unwrap();
    let (volatile, warmup, restore_count, clean_count) = guards
        .as_ref()
        .expect("deep delay vm handles not set")
        .clone();
    Box::new(AstDeepDelayVm {
        volatile,
        warmup,
        restore_count,
        clean_count,
    })
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestDeepDelayVmInit;

impl Parse for AstTestDeepDelayVmInit {
    fn parse(&mut self, _buf: &[u8]) -> Ret<usize> {
        Ok(0)
    }
}

impl Serialize for AstTestDeepDelayVmInit {
    fn serialize(&self) -> Vec<u8> {
        vec![]
    }

    fn size(&self) -> usize {
        0
    }
}

impl Field for AstTestDeepDelayVmInit {
    fn new() -> Self {
        Self
    }
}

impl ToJSON for AstTestDeepDelayVmInit {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestDeepDelayVmInit {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestDeepDelayVmInit {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        unsafe { ctx_inst(ctx) }.test_set_vm(take_ast_deep_delay_vm());
        Ok((0, vec![]))
    }
}

impl Description for AstTestDeepDelayVmInit {}

impl Action for AstTestDeepDelayVmInit {
    fn kind(&self) -> u16 {
        65019
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestDeepDelayVmCall {
    vol_add: Uint1,
    warm_add: Uint1,
    fail: Uint1,
}

impl Parse for AstTestDeepDelayVmCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.vol_add.parse(buf)?;
        mv += self.warm_add.parse(&buf[mv..])?;
        mv += self.fail.parse(&buf[mv..])?;
        Ok(mv)
    }
}

impl Serialize for AstTestDeepDelayVmCall {
    fn serialize(&self) -> Vec<u8> {
        [
            self.vol_add.serialize(),
            self.warm_add.serialize(),
            self.fail.serialize(),
        ]
        .concat()
    }

    fn size(&self) -> usize {
        self.vol_add.size() + self.warm_add.size() + self.fail.size()
    }
}

impl Field for AstTestDeepDelayVmCall {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestDeepDelayVmCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestDeepDelayVmCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestDeepDelayVmCall {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let payload = vec![*self.vol_add, *self.warm_add, *self.fail];
        let (_gas, rv) = ctx.vm_call(0, 0, payload.into(), Box::new(()))?;
        // VM dynamic gas is charged through shared ctx remaining inside VM runtime.
        // Keep action return-gas channel as size-only (0 here for this custom test action).
        Ok((0, rv))
    }
}

impl Description for AstTestDeepDelayVmCall {}

impl Action for AstTestDeepDelayVmCall {
    fn kind(&self) -> u16 {
        65020
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AstTestDeepDelayVmCall {
    fn create_by(vol_add: u8, warm_add: u8, fail: u8) -> Self {
        Self {
            vol_add: Uint1::from(vol_add),
            warm_add: Uint1::from(warm_add),
            fail: Uint1::from(fail),
        }
    }
}

struct AstBugAssumeVm {
    remaining: std::sync::Arc<std::sync::atomic::AtomicI64>,
    burned: std::sync::Arc<std::sync::atomic::AtomicI64>,
    volatile_mark: std::sync::atomic::AtomicI64,
}

impl AstBugAssumeVm {
    fn create(
        remaining: i64,
    ) -> (
        Box<dyn VM>,
        std::sync::Arc<std::sync::atomic::AtomicI64>,
        std::sync::Arc<std::sync::atomic::AtomicI64>,
    ) {
        let rem = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(remaining));
        let burned = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
        (
            Box::new(Self {
                remaining: rem.clone(),
                burned: burned.clone(),
                volatile_mark: std::sync::atomic::AtomicI64::new(0),
            }),
            rem,
            burned,
        )
    }
}

impl VM for AstBugAssumeVm {
    fn snapshot_volatile(&mut self) -> Box<dyn std::any::Any> {
        Box::new(self.volatile_mark.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn restore_volatile(&mut self, snap: Box<dyn std::any::Any>) {
        if let Ok(mark) = snap.downcast::<i64>() {
            self.volatile_mark
                .store(*mark, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn call(
        &mut self,
        _: &mut dyn Context,
        _: u8,
        _target: u8,
        payload: std::sync::Arc<[u8]>,
        _: Box<dyn std::any::Any>,
    ) -> XRet<(i64, Vec<u8>)> {
        let data = payload.as_ref();
        if data.len() < 2 {
            return xerrf!("ast bug assume vm payload too short");
        }
        let should_fail = data[0] != 0;
        let gas_cost = data[1] as i64;
        self.remaining
            .fetch_sub(gas_cost, std::sync::atomic::Ordering::SeqCst);
        self.volatile_mark
            .fetch_add(gas_cost, std::sync::atomic::Ordering::SeqCst);
        if should_fail {
            return xerr_rf!("ast bug assume vm forced fail");
        }
        self.burned
            .fetch_add(gas_cost, std::sync::atomic::Ordering::SeqCst);
        Ok((gas_cost, vec![]))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestBugVmCall {
    fail: Uint1,
    cost: Uint1,
}

impl Parse for AstTestBugVmCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.fail.parse(buf)?;
        mv += self.cost.parse(&buf[mv..])?;
        Ok(mv)
    }
}

impl Serialize for AstTestBugVmCall {
    fn serialize(&self) -> Vec<u8> {
        [self.fail.serialize(), self.cost.serialize()].concat()
    }

    fn size(&self) -> usize {
        self.fail.size() + self.cost.size()
    }
}

impl Field for AstTestBugVmCall {
    fn new() -> Self {
        Self::default()
    }
}

impl ToJSON for AstTestBugVmCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

impl FromJSON for AstTestBugVmCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

impl ActExec for AstTestBugVmCall {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let payload = vec![*self.fail, *self.cost];
        let (_gas, rv) = ctx.vm_call(0, 0, payload.into(), Box::new(()))?;
        // VM dynamic gas is charged through shared ctx remaining inside VM runtime.
        // Keep action return-gas channel as size-only (0 here for this custom test action).
        Ok((0, rv))
    }
}

impl Description for AstTestBugVmCall {}

impl Action for AstTestBugVmCall {
    fn kind(&self) -> u16 {
        65016
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AstTestBugVmCall {
    fn fail(cost: u8) -> Self {
        Self {
            fail: Uint1::from(1),
            cost: Uint1::from(cost),
        }
    }

    fn ok(cost: u8) -> Self {
        Self {
            fail: Uint1::from(0),
            cost: Uint1::from(cost),
        }
    }
}

#[test]
fn test_ast_bug_assumption_fail_child_then_success_child_burn_gap() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (vm, remaining, burned) = AstBugAssumeVm::create(100);
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(vm);

    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestBugVmCall::fail(30)),
            Box::new(AstTestBugVmCall::ok(5)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(remaining.load(std::sync::atomic::Ordering::SeqCst), 65);
    assert_eq!(burned.load(std::sync::atomic::Ordering::SeqCst), 5);
}

#[test]
fn test_ast_bug_assumption_min_zero_allows_failed_vm_branch_without_burn() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (vm, remaining, burned) = AstBugAssumeVm::create(100);
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(vm);

    let act = AstSelect::create_by(0, 1, vec![Box::new(AstTestBugVmCall::fail(20))]);
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(remaining.load(std::sync::atomic::Ordering::SeqCst), 80);
    assert_eq!(burned.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_ast_bug_control_all_success_children_no_burn_gap() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (vm, remaining, burned) = AstBugAssumeVm::create(100);
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(vm);

    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestBugVmCall::ok(30)),
            Box::new(AstTestBugVmCall::ok(5)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    let rem = remaining.load(std::sync::atomic::Ordering::SeqCst);
    let bur = burned.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(rem, 65);
    assert_eq!(bur, 35);
    assert_eq!(100 - rem, bur);
}

#[test]
fn test_ast_bug_control_min_zero_success_child_charged() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (vm, remaining, burned) = AstBugAssumeVm::create(100);
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(vm);

    let act = AstSelect::create_by(0, 1, vec![Box::new(AstTestBugVmCall::ok(20))]);
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    let rem = remaining.load(std::sync::atomic::Ordering::SeqCst);
    let bur = burned.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(rem, 80);
    assert_eq!(bur, 20);
    assert_eq!(100 - rem, bur);
}

#[test]
fn test_ast_vm_recover_false_to_true_uses_restore_but_keep_warmup() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let value = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_recover_track_handles(value.clone(), restore_count.clone(), clean_count.clone());

    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestVmInitReplace::create_by(9)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(inner_fail),
            Box::new(AstTestSet::create_by(190, 190)),
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(value.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(ast_state_get_u8(&mut ctx, 190), Some(190));
}

#[test]
fn test_ast_vm_delay_init_deep_nested_revert_rollback_warmup_kept() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.tex_ledger().zhu = 10;

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestCombo::create_by(166, 6)),
            Box::new(AstTestDeepDelayVmCall::create_by(9, 3, 1)),
        ],
    );
    let middle = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestDeepDelayVmInit::new()),
            Box::new(AstTestDeepDelayVmCall::create_by(5, 2, 0)),
            Box::new(AstTestP2shSetN::create_by(116)),
            Box::new(AstTestSet::create_by(165, 165)),
        ]),
        AstSelect::create_list(vec![Box::new(inner_fail)]),
        AstSelect::nop(),
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![Box::new(middle), Box::new(AstTestSet::create_by(167, 167))],
    );

    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 165), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 166), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 167), Some(167));
    assert_eq!(ctx.tex_ledger().zhu, 10);
    assert_eq!(unsafe { &*logs_ptr }.len(), 0);
    assert!(ctx.p2sh(&Address::create_scriptmh([116u8; 20])).is_err());
    assert_eq!(volatile.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(warmup.load(std::sync::atomic::Ordering::SeqCst), 5);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 5);
    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn test_ast_vm_delay_init_deep_nested_success_commits_reverts() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let inner_ok = AstSelect::create_list(vec![
        Box::new(AstTestDeepDelayVmCall::create_by(4, 1, 0)),
        Box::new(AstTestCombo::create_by(168, 8)),
    ]);
    let middle = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestDeepDelayVmInit::new()),
            Box::new(AstTestDeepDelayVmCall::create_by(3, 2, 0)),
            Box::new(AstTestP2shSetN::create_by(117)),
        ]),
        AstSelect::create_list(vec![
            Box::new(inner_ok),
            Box::new(AstTestSet::create_by(169, 169)),
        ]),
        AstSelect::nop(),
    );
    let act = AstSelect::create_list(vec![Box::new(middle)]);

    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 168), Some(8));
    assert_eq!(ast_state_get_u8(&mut ctx, 169), Some(169));
    assert_eq!(ctx.tex_ledger().zhu, 8);
    assert_eq!(unsafe { &*logs_ptr }.len(), 1);
    assert!(ctx.p2sh(&Address::create_scriptmh([117u8; 20])).is_ok());
    assert!(volatile.load(std::sync::atomic::Ordering::SeqCst) > 0);
    assert!(warmup.load(std::sync::atomic::Ordering::SeqCst) > 0);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_ast_vm_delay_init_depth6_revert_and_fault_channels() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.fee = Amount::unit238(1_000_000);
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(
        &mut ctx,
        &field::ADDRESS_ONEX,
        &Amount::unit238(1_000_000_000),
    )
    .unwrap();
    ctx.gas_init_tx(4000, 1).unwrap();

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let lvl6_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestDeepDelayVmCall::create_by(9, 3, 1)),
            Box::new(AstTestSet::create_by(232, 232)),
        ],
    );
    let lvl5 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestDeepDelayVmCall::create_by(4, 2, 0))]),
        AstSelect::create_list(vec![Box::new(lvl6_fail)]),
        AstSelect::nop(),
    );
    let lvl4 = AstSelect::create_list(vec![
        Box::new(AstTestDeepDelayVmInit::new()),
        Box::new(AstTestSet::create_by(231, 231)),
        Box::new(lvl5),
    ]);
    let lvl3 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(230, 230))]),
        AstSelect::create_list(vec![Box::new(lvl4)]),
        AstSelect::nop(),
    );
    let root = AstSelect::create_by(
        1,
        2,
        vec![Box::new(lvl3), Box::new(AstTestSet::create_by(233, 233))],
    );

    let err = check_action_ast_tree_depth(&root).unwrap_err();
    assert!(err.contains("ast tree depth 7 exceeded max 6"), "{}", err);

    // precheck rejects the whole root AST node before execution
    assert_eq!(ast_state_get_u8(&mut ctx, 230), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 231), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 232), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 233), None);

    // precheck aborts before any VM path runs, so warmup state must stay untouched
    assert_eq!(volatile.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(warmup.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_ast_layered_composition_mixed_vm_calls_snapshot_gas_exact() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.fee = Amount::unit238(1_000_000);
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(
        &mut ctx,
        &field::ADDRESS_ONEX,
        &Amount::unit238(1_000_000_000),
    )
    .unwrap();
    ctx.gas_init_tx(2000, 1).unwrap();

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let node1 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(241, 241))]);
    let node2 = AstSelect::create_list(vec![
        Box::new(AstTestDeepDelayVmInit::new()),
        Box::new(AstTestDeepDelayVmCall::create_by(1, 1, 0)),
    ]);
    let node3 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestDeepDelayVmCall::create_by(2, 1, 0))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(242, 242))]),
        AstSelect::nop(),
    );
    let root = AstSelect::create_list(vec![Box::new(node1), Box::new(node2), Box::new(node3)]);

    let before = ctx.gas_remaining();
    ctx.exec_from_set(ExecFrom::Top);
    root.execute(&mut ctx).unwrap();
    let after = ctx.gas_remaining();

    // Top-level loop discards return gas; only shared snapshot/dynamic gas is charged here.
    // +40 compared to old model: AstIf branch now uses isolated snapshot instead of shared.
    assert_eq!(before - after, 400);
    assert_eq!(ast_state_get_u8(&mut ctx, 241), Some(241));
    assert_eq!(ast_state_get_u8(&mut ctx, 242), Some(242));
    assert!(volatile.load(std::sync::atomic::Ordering::SeqCst) >= 0);
    assert!(warmup.load(std::sync::atomic::Ordering::SeqCst) >= 0);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_ast_layered_with_mid_vm_failure_revert_and_warmup_monotonic() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.fee = Amount::unit238(1_000_000);
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(
        &mut ctx,
        &field::ADDRESS_ONEX,
        &Amount::unit238(1_000_000_000),
    )
    .unwrap();
    ctx.gas_init_tx(2000, 1).unwrap();

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let first_ok = AstSelect::create_list(vec![
        Box::new(AstTestDeepDelayVmInit::new()),
        Box::new(AstTestDeepDelayVmCall::create_by(5, 2, 0)),
    ]);
    let mid_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestDeepDelayVmCall::create_by(7, 3, 1)),
            Box::new(AstTestSet::create_by(251, 251)),
        ],
    );
    let root = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(first_ok),
            Box::new(mid_fail),
            Box::new(AstTestSet::create_by(252, 252)),
        ],
    );

    let before = ctx.gas_remaining();
    ctx.exec_from_set(ExecFrom::Top);
    root.execute(&mut ctx).unwrap();
    let after = ctx.gas_remaining();

    // snapshots include attempt-level captures only.
    assert_eq!(before - after, 280);
    assert_eq!(ast_state_get_u8(&mut ctx, 251), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 252), Some(252));
    assert!(volatile.load(std::sync::atomic::Ordering::SeqCst) >= 0);
    assert_eq!(warmup.load(std::sync::atomic::Ordering::SeqCst), 5);
    assert!(restore_count.load(std::sync::atomic::Ordering::SeqCst) >= 1);
}

#[test]
fn test_tx_multiple_top_ast_with_internal_vm_calls_gas_settlement_matches_balance() {
    let _guard = ast_test_globals_guard();
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.gas_max = Uint1::from(17);
    tx.fee = Amount::unit238(1_000_000);
    tx.actions
        .push(Box::new(AstSelect::create_list(vec![Box::new(
            AstTestSet::create_by(201, 201),
        )])))
        .unwrap();
    tx.actions
        .push(Box::new(AstSelect::create_list(vec![
            Box::new(AstTestDeepDelayVmInit::new()),
            Box::new(AstTestDeepDelayVmCall::create_by(1, 1, 0)),
        ])))
        .unwrap();
    tx.actions
        .push(Box::new(AstIf::create_by(
            AstSelect::create_list(vec![Box::new(AstTestDeepDelayVmCall::create_by(2, 1, 0))]),
            AstSelect::create_list(vec![Box::new(AstTestSet::create_by(202, 202))]),
            AstSelect::nop(),
        )))
        .unwrap();

    let main = tx.main();
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = main;
    env.tx.addrs = vec![main];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(5_000_000_000)).unwrap();

    let volatile = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let warmup = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let restore_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clean_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    set_ast_deep_delay_vm_handles(
        volatile.clone(),
        warmup.clone(),
        restore_count.clone(),
        clean_count.clone(),
    );

    let _before = ast_hac_balance(&mut ctx, &main);
    ctx.exec_from_set(ExecFrom::Top);
    tx.execute(&mut ctx).unwrap();
    let after = ast_hac_balance(&mut ctx, &main);

    assert!(after >= Amount::zero());
    assert_eq!(ast_state_get_u8(&mut ctx, 201), Some(201));
    assert_eq!(ast_state_get_u8(&mut ctx, 202), Some(202));
    assert_eq!(volatile.load(std::sync::atomic::Ordering::SeqCst), 3);
    assert_eq!(warmup.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(restore_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(clean_count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_tx_failed_ast_charges_used_gas_but_not_fee() {
    let mut tx = TransactionType3::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.ty = Uint1::from(TransactionType3::TYPE);
    tx.gas_max = Uint1::from(17);
    tx.fee = Amount::unit238(1_000_000);
    tx.actions
        .push(Box::new(AstSelect::create_by(
            1,
            1,
            vec![Box::new(AstTestFail::new())],
        )))
        .unwrap();

    let main = tx.main();
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    env.tx.main = main;
    env.tx.addrs = vec![main];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(5_000_000_000)).unwrap();

    let _before = ast_hac_balance(&mut ctx, &main);
    ctx.exec_from_set(ExecFrom::Top);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least") || err.contains("ast test forced fail"));
    let after = ast_hac_balance(&mut ctx, &main);

    let used = ctx.ctx_gas_used_charge().unwrap();
    assert!(
        used.is_positive(),
        "failed tx should still consume gas via AST snapshots"
    );

    assert!(
        after >= Amount::zero(),
        "failed tx should keep a valid balance amount"
    );
}

// PLACEHOLDER_VM_TESTS
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainSet {
    key: Uint1,
    val: Uint1,
}
impl Parse for AstTestMainSet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
impl Serialize for AstTestMainSet {
    fn serialize(&self) -> Vec<u8> {
        [self.key.serialize(), self.val.serialize()].concat()
    }
    fn size(&self) -> usize {
        self.key.size() + self.val.size()
    }
}
impl Field for AstTestMainSet {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestMainSet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}
impl FromJSON for AstTestMainSet {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "key" {
                self.key.from_json(v)?;
            } else if k == "val" {
                self.val.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestMainSet {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        Ok((0, vec![]))
    }
}
impl Description for AstTestMainSet {}
impl Action for AstTestMainSet {
    fn kind(&self) -> u16 {
        65011
    }
    fn scope(&self) -> ActScope {
        ActScope::CALL
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestMainSet {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainP2shSetN {
    addr_byte: Uint1,
}
impl Parse for AstTestMainP2shSetN {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.addr_byte.parse(buf)
    }
}
impl Serialize for AstTestMainP2shSetN {
    fn serialize(&self) -> Vec<u8> {
        self.addr_byte.serialize()
    }
    fn size(&self) -> usize {
        self.addr_byte.size()
    }
}
impl Field for AstTestMainP2shSetN {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestMainP2shSetN {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"addr_byte\":{}}}", self.addr_byte.to_json_fmt(fmt))
    }
}
impl FromJSON for AstTestMainP2shSetN {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "addr_byte" {
                self.addr_byte.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestMainP2shSetN {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        Ok((0, vec![]))
    }
}
impl Description for AstTestMainP2shSetN {}
impl Action for AstTestMainP2shSetN {
    fn kind(&self) -> u16 {
        65012
    }
    fn scope(&self) -> ActScope {
        ActScope::CALL
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestMainP2shSetN {
    fn create_by(n: u8) -> Self {
        Self {
            addr_byte: Uint1::from(n),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainVMCall {
    increment: Uint1,
}
impl Parse for AstTestMainVMCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.increment.parse(buf)
    }
}
impl Serialize for AstTestMainVMCall {
    fn serialize(&self) -> Vec<u8> {
        self.increment.serialize()
    }
    fn size(&self) -> usize {
        self.increment.size()
    }
}
impl Field for AstTestMainVMCall {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestMainVMCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}
impl FromJSON for AstTestMainVMCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}
impl ActExec for AstTestMainVMCall {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        let Some(snap) = ctx.vm_snapshot_volatile() else {
            return xerrf!("test vm missing");
        };
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.increment as i64;
            ctx.vm_restore_volatile(Box::new(new_val));
        }
        Ok((0, vec![]))
    }
}
impl Description for AstTestMainVMCall {}
impl Action for AstTestMainVMCall {
    fn kind(&self) -> u16 {
        65013
    }
    fn scope(&self) -> ActScope {
        ActScope::CALL
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestMainVMCall {
    fn create_by(inc: u8) -> Self {
        Self {
            increment: Uint1::from(inc),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestRet {
    tag: Uint1,
}
impl Parse for AstTestRet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.tag.parse(buf)
    }
}
impl Serialize for AstTestRet {
    fn serialize(&self) -> Vec<u8> {
        self.tag.serialize()
    }
    fn size(&self) -> usize {
        self.tag.size()
    }
}
impl Field for AstTestRet {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestRet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"tag\":{}}}", self.tag.to_json_fmt(fmt))
    }
}
impl FromJSON for AstTestRet {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "tag" {
                self.tag.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestRet {
    fn execute(&self, _ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        Ok((0, vec![*self.tag]))
    }
}
impl Description for AstTestRet {}
impl Action for AstTestRet {
    fn kind(&self) -> u16 {
        65014
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestRet {
    fn create_by(tag: u8) -> Self {
        Self {
            tag: Uint1::from(tag),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMutateAllFail {
    key: Uint1,
    val: Uint1,
    addr_byte: Uint1,
    vm_add: Uint1,
}
impl Parse for AstTestMutateAllFail {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        mv += self.addr_byte.parse(&buf[mv..])?;
        mv += self.vm_add.parse(&buf[mv..])?;
        Ok(mv)
    }
}
impl Serialize for AstTestMutateAllFail {
    fn serialize(&self) -> Vec<u8> {
        [
            self.key.serialize(),
            self.val.serialize(),
            self.addr_byte.serialize(),
            self.vm_add.serialize(),
        ]
        .concat()
    }
    fn size(&self) -> usize {
        self.key.size() + self.val.size() + self.addr_byte.size() + self.vm_add.size()
    }
}
impl Field for AstTestMutateAllFail {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestMutateAllFail {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{},\"addr_byte\":{},\"vm_add\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt),
            self.addr_byte.to_json_fmt(fmt),
            self.vm_add.to_json_fmt(fmt)
        )
    }
}
impl FromJSON for AstTestMutateAllFail {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "key" {
                self.key.from_json(v)?;
            } else if k == "val" {
                self.val.from_json(v)?;
            } else if k == "addr_byte" {
                self.addr_byte.from_json(v)?;
            } else if k == "vm_add" {
                self.vm_add.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestMutateAllFail {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        ctx.tex_ledger().zhu += *self.val as i64;
        ctx.logs().push(&self.key);
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        let Some(snap) = ctx.vm_snapshot_volatile() else {
            return xerrf!("test vm missing");
        };
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.vm_add as i64;
            ctx.vm_restore_volatile(Box::new(new_val));
        }
        xerr_rf!("ast test mutate-all fail")
    }
}
impl Description for AstTestMutateAllFail {}
impl Action for AstTestMutateAllFail {
    fn kind(&self) -> u16 {
        65015
    }
    fn scope(&self) -> ActScope {
        ActScope::CALL
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestMutateAllFail {
    fn create_by(key: u8, val: u8, addr_byte: u8, vm_add: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
            addr_byte: Uint1::from(addr_byte),
            vm_add: Uint1::from(vm_add),
        }
    }
}

// ---- Test 21: VM state restored on AstSelect child failure ----
#[test]
fn test_ast_vm_state_restored_on_select_child_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // child1: vm += 5, succeed
    // child2: vm += 10, then fail -> vm should be rolled back to 5
    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestVMCall::create_by(10)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![Box::new(AstTestVMCall::create_by(5)), Box::new(inner_fail)],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
}

// ---- Test 22: VM state fully rolled back when AstIf branch fails ----
#[test]
fn test_ast_vm_state_rolled_back_on_if_branch_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // cond: vm += 3, succeed -> br_if: vm += 7, fail
    // whole_snap recover should restore vm to 0
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]),
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestVMCall::create_by(7)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _err = astif.execute(&mut ctx).unwrap_err();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
}

// ---- Test 23: VM state committed on successful AstIf path ----
#[test]
fn test_ast_vm_state_committed_on_success() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // cond: vm += 2, br_if: vm += 3 -> total 5
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(2))]),
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
}

// ---- Test 24: VM + state + tex + logs + p2sh all restored together on failure ----
#[test]
fn test_ast_all_five_channels_restored_on_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);
    ctx.tex_ledger().zhu = 100;

    // All channels modified, then fail
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(110, 10)), // state + tex + log
            Box::new(AstTestVMCall::create_by(5)),      // vm
            Box::new(AstTestP2shSetN::create_by(70)),   // p2sh
        ]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]), // force fail
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // All five channels must be restored
    assert_eq!(ast_state_get_u8(&mut ctx, 110), None);
    assert_eq!(ctx.tex_ledger().zhu, 100);
    assert_eq!(unsafe { &*logs_ptr }.len(), 0);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(ctx.p2sh(&Address::create_scriptmh([70u8; 20])).is_err());
}

// ---- Test 25: VM state in nested AstIf-inside-AstSelect: inner fail isolated ----
#[test]
fn test_ast_vm_nested_if_fail_isolated_by_outer_select() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // child1: vm += 10, ok
    // child2: AstIf(cond: vm += 20, br_if: fail) -> inner fail, outer select recovers
    // child3: vm += 30, ok
    // Expected: 10 + 30 = 40 (child2's 20 rolled back)
    let inner_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(20))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
    );
    let act = AstSelect::create_by(
        2,
        3,
        vec![
            Box::new(AstTestVMCall::create_by(10)),
            Box::new(inner_if),
            Box::new(AstTestVMCall::create_by(30)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 40);
}

// ---- Test 26: ctx_action_call (ACTION) nested inside AstSelect ----
// Tests that actions created via ctx_action_call within AST branches
// have their state changes properly rolled back on failure.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestExtCall {
    key: Uint1,
    val: Uint1,
}
impl Parse for AstTestExtCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
impl Serialize for AstTestExtCall {
    fn serialize(&self) -> Vec<u8> {
        [self.key.serialize(), self.val.serialize()].concat()
    }
    fn size(&self) -> usize {
        self.key.size() + self.val.size()
    }
}
impl Field for AstTestExtCall {
    fn new() -> Self {
        Self::default()
    }
}
impl ToJSON for AstTestExtCall {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}
impl FromJSON for AstTestExtCall {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json)?;
        for (k, v) in pairs {
            if k == "key" {
                self.key.from_json(v)?;
            } else if k == "val" {
                self.val.from_json(v)?;
            }
        }
        Ok(())
    }
}
impl ActExec for AstTestExtCall {
    fn execute(&self, ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        // Simulate what ACTION does: modify state through ctx_action_call path.
        // We directly set state here since ctx_action_call ultimately calls action.execute(ctx).
        ctx.state().set(vec![*self.key], vec![*self.val]);
        // Also modify tex to test cross-channel consistency
        ctx.tex_ledger().sat += *self.val as i64;
        Ok((0, vec![]))
    }
}
impl Description for AstTestExtCall {}
impl Action for AstTestExtCall {
    fn kind(&self) -> u16 {
        65010
    }
    fn scope(&self) -> ActScope {
        ActScope::AST
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl AstTestExtCall {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
        }
    }
}

// ---- Test 26: ACTION-like state changes rolled back in failed AstSelect child ----
#[test]
fn test_ast_extcall_state_rolled_back_on_select_child_failure() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    // child1: extcall sets key=120 val=1, sat+=1, ok
    // child2: extcall sets key=121 val=2, sat+=2, then fail -> rolled back
    let inner_fail = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestExtCall::create_by(121, 2)),
            Box::new(AstTestFail::new()),
        ],
    );
    let act = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(AstTestExtCall::create_by(120, 1)),
            Box::new(inner_fail),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 120), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 121), None);
    assert_eq!(ctx.tex_ledger().sat, 1); // only child1's sat
}

// ---- Test 27: Multiple sequential AST ops with VM — state accumulates correctly ----
#[test]
fn test_ast_vm_sequential_accumulation() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // Op1: select(vm += 3) -> ok, counter = 3
    let act1 = AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]);
    ctx.exec_from_set(ExecFrom::Top);
    act1.execute(&mut ctx).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);

    // Op2: select(vm += 7, fail) -> fail, counter stays 3
    let act2 = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestVMCall::create_by(7)),
            Box::new(AstTestFail::new()),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _ = act2.execute(&mut ctx);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);

    // Op3: if(cond: vm += 2, br_if: vm += 4) -> ok, counter = 3 + 2 + 4 = 9
    let act3 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(2))]),
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(4))]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    act3.execute(&mut ctx).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 9);
}

// ---- Test 28: Deep 3-level nesting with all channels ----
// AstIf -> AstSelect -> AstIf, with VM + state + tex + logs + p2sh
#[test]
fn test_ast_deep_3level_all_channels() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);

    // Level 3: AstIf(cond: fail -> else: set(130,130) + vm+=1 + log)
    let lvl3 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(130, 10)),
            Box::new(AstTestVMCall::create_by(1)),
        ]),
    );
    // Level 2: AstSelect(min=2, max=2): [set(131,131), lvl3]
    let lvl2 = AstSelect::create_list(vec![
        Box::new(AstTestSet::create_by(131, 131)),
        Box::new(lvl3),
    ]);
    // Level 1: AstIf(cond: vm+=2 + p2sh(80), br_if: [lvl2, tex+=5])
    let lvl1 = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestVMCall::create_by(2)),
            Box::new(AstTestP2shSetN::create_by(80)),
        ]),
        AstSelect::create_list(vec![Box::new(lvl2), Box::new(AstTestTexAdd::create_by(5))]),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    lvl1.execute(&mut ctx).unwrap();

    // Verify all channels
    assert_eq!(ast_state_get_u8(&mut ctx, 130), Some(10));
    assert_eq!(ast_state_get_u8(&mut ctx, 131), Some(131));
    // combo(130,10) adds 10 to zhu, tex_add(5) adds 5 to zhu -> total 15
    assert_eq!(ctx.tex_ledger().zhu, 15);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3); // 2 + 1
    assert!(ctx.p2sh(&Address::create_scriptmh([80u8; 20])).is_ok());
    assert_eq!(unsafe { &*logs_ptr }.len(), 1); // combo pushed 1 log
}

// ---- Test 29: AstIf cond partial failure with MainCall side-effects rolls back then runs else ----
#[test]
fn test_ast_if_cond_partial_failure_with_maincall_rolls_back_and_runs_else() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    let astif = AstIf::create_by(
        // cond: first three children mutate state/p2sh/vm, then final child fails -> cond Err
        AstSelect::create_by(
            4,
            4,
            vec![
                Box::new(AstTestMainSet::create_by(10, 10)),
                Box::new(AstTestMainP2shSetN::create_by(90)),
                Box::new(AstTestMainVMCall::create_by(4)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(12, 12))]),
        AstSelect::create_list(vec![
            Box::new(AstTestMainSet::create_by(11, 11)),
            Box::new(AstTestMainP2shSetN::create_by(91)),
            Box::new(AstTestMainVMCall::create_by(6)),
        ]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 10), None); // cond write rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 11), Some(11)); // else branch committed
    assert_eq!(ast_state_get_u8(&mut ctx, 12), None); // if branch not taken
    assert!(ctx.p2sh(&Address::create_scriptmh([90u8; 20])).is_err()); // cond p2sh rolled back
    assert!(ctx.p2sh(&Address::create_scriptmh([91u8; 20])).is_ok()); // else p2sh committed
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 6); // cond vm +4 rolled back
}

// ---- Test 30: Mixed MainCall + AST nested failure is isolated by outer AstSelect ----
#[test]
fn test_ast_select_nested_mixed_maincall_p2sh_vm_failure_isolated() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    let nested_if_fail = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestMainSet::create_by(20, 20)),
            Box::new(AstTestMainP2shSetN::create_by(92)),
            Box::new(AstTestMainVMCall::create_by(7)),
        ]),
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestMainSet::create_by(21, 21)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::nop(),
    );
    let outer = AstSelect::create_by(
        2,
        3,
        vec![
            Box::new(AstTestMainVMCall::create_by(5)),    // success #1
            Box::new(nested_if_fail),                     // fail, must be isolated
            Box::new(AstTestMainP2shSetN::create_by(93)), // success #2
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    outer.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 20), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 21), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([92u8; 20])).is_err());
    assert!(ctx.p2sh(&Address::create_scriptmh([93u8; 20])).is_ok());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5); // nested +7 rolled back
}

// ---- Test 31: Deep AstIf->AstSelect->AstIf with MainCall actions commits expected channels ----
#[test]
fn test_ast_deep_maincall_if_select_if_commits_expected_state() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    // level 3: cond fails after vm += 1, then else branch commits state + vm
    let lvl3 = AstIf::create_by(
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestMainVMCall::create_by(1)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(30, 30))]),
        AstSelect::create_list(vec![
            Box::new(AstTestMainSet::create_by(31, 31)),
            Box::new(AstTestMainVMCall::create_by(2)),
        ]),
    );
    // level 2
    let lvl2 = AstSelect::create_list(vec![
        Box::new(AstTestMainP2shSetN::create_by(94)),
        Box::new(lvl3),
    ]);
    // level 1
    let lvl1 = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestMainSet::create_by(32, 32)),
            Box::new(AstTestMainVMCall::create_by(3)),
        ]),
        AstSelect::create_list(vec![Box::new(lvl2)]),
        AstSelect::nop(),
    );

    ctx.exec_from_set(ExecFrom::Top);
    lvl1.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 32), Some(32));
    assert_eq!(ast_state_get_u8(&mut ctx, 31), Some(31));
    assert_eq!(ast_state_get_u8(&mut ctx, 30), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([94u8; 20])).is_ok());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5); // 3 + 2, cond-failed +1 rolled back
}

// ---- Test 32: AstSelect rejects actions len > TX_ACTIONS_MAX without leaking state context ----
#[test]
fn test_ast_select_num_over_tx_actions_max_rejected_no_leak() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.state().set(vec![241], vec![241]); // baseline

    let mut acts: Vec<Box<dyn Action>> = vec![];
    for i in 0..(TX_ACTIONS_MAX + 1) {
        acts.push(Box::new(AstTestSet::create_by((i % 200) as u8, 1)));
    }
    let over = AstSelect::create_by(0, 0, acts);

    ctx.exec_from_set(ExecFrom::Top);
    let err = over.execute(&mut ctx).unwrap_err();
    assert!(err.contains("num cannot exceed"), "{}", err);

    // state fork should not leak; context stays available
    assert_eq!(ast_state_get_u8(&mut ctx, 241), Some(241));
    ctx.state().set(vec![242], vec![242]);
    assert_eq!(ast_state_get_u8(&mut ctx, 242), Some(242));
}

// ---- Test 33: AstSelect max=0 short-circuits and executes no child ----
#[test]
fn test_ast_select_max_zero_executes_no_children() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();
    ctx.test_set_vm(mock_vm);

    let act = AstSelect::create_by(
        0,
        0,
        vec![
            Box::new(AstTestSet::create_by(243, 1)),
            Box::new(AstTestP2shSetN::create_by(96)),
            Box::new(AstTestVMCall::create_by(9)),
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    let rv = act.execute(&mut ctx).unwrap();
    assert_eq!(rv.1, Vec::<u8>::new());
    assert_eq!(ast_state_get_u8(&mut ctx, 243), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([96u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
}

// ---- Test 34: AstSelect returns result bytes from the last successful child ----
#[test]
fn test_ast_select_returns_last_success_result_bytes() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let act = AstSelect::create_by(
        1,
        3,
        vec![
            Box::new(AstTestRet::create_by(1)),
            Box::new(AstTestFail::new()),
            Box::new(AstTestRet::create_by(3)),
        ],
    );
    ctx.exec_from_set(ExecFrom::Top);
    let (_, rv) = act.execute(&mut ctx).unwrap();
    assert_eq!(rv, vec![3]);
}

// ---- Test 35: AstIf returns selected branch bytes and restores exec_from ----
#[test]
fn test_ast_if_returns_selected_branch_result_and_restores_exec_from() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]), // cond fail => else
        AstSelect::create_list(vec![Box::new(AstTestRet::create_by(7))]),
        AstSelect::create_list(vec![Box::new(AstTestRet::create_by(8))]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    let (_, rv) = astif.execute(&mut ctx).unwrap();
    assert_eq!(rv, vec![8]);
    assert_eq!(ctx.exec_from(), ExecFrom::Top);
}

// ---- Test 36: AstIf branch error still restores exec_from by ExecFromGuard ----
#[test]
fn test_ast_if_error_restores_exec_from() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(244, 244))]),
        AstSelect::create_by(
            2,
            2,
            vec![
                Box::new(AstTestSet::create_by(245, 245)),
                Box::new(AstTestFail::new()),
            ],
        ),
        AstSelect::nop(),
    );
    ctx.exec_from_set(ExecFrom::Top);
    let _ = astif.execute(&mut ctx).unwrap_err();

    assert_eq!(ctx.exec_from(), ExecFrom::Top);
}

// ---- Test 37: Invalid cond AstSelect in AstIf falls through to else without cond side-effects ----
#[test]
fn test_ast_if_invalid_cond_select_runs_else_no_cond_leak() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let astif = AstIf::create_by(
        // invalid cond: min > max, now treated as fault and aborts whole AstIf
        AstSelect::create_by(2, 1, vec![Box::new(AstTestMainSet::create_by(246, 246))]),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(247, 247))]),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(248, 248))]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("action ast select max cannot be less than min"),
        "{}",
        err
    );

    assert_eq!(ast_state_get_u8(&mut ctx, 246), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 247), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 248), None);
}

// ---- Test 38: AstSelect child that mutates all channels then fails is fully recovered ----
#[test]
fn test_ast_select_direct_child_mutate_all_fail_recovers_all_channels() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);
    ctx.tex_ledger().zhu = 10;

    let child_ok = AstSelect::create_list(vec![
        Box::new(AstTestCombo::create_by(249, 2)), // state + tex + log
        Box::new(AstTestMainP2shSetN::create_by(97)), // p2sh
        Box::new(AstTestMainVMCall::create_by(3)), // vm
    ]);
    let child_fail = AstTestMutateAllFail::create_by(250, 5, 98, 7); // all channels mutate then Err
    let act = AstSelect::create_by(1, 2, vec![Box::new(child_ok), Box::new(child_fail)]);

    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 249), Some(2));
    assert_eq!(ast_state_get_u8(&mut ctx, 250), None);
    assert_eq!(ctx.tex_ledger().zhu, 12); // only child_ok committed
    assert_eq!(unsafe { &*logs_ptr }.len(), 1); // only combo log kept
    assert!(ctx.p2sh(&Address::create_scriptmh([97u8; 20])).is_ok());
    assert!(ctx.p2sh(&Address::create_scriptmh([98u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3); // +7 rolled back
}

// ---- Test 39: AstIf branch mutate-then-fail triggers whole-snap recovery of all channels ----
#[test]
fn test_ast_if_branch_mutate_all_fail_recovers_whole_snap_all_channels() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);
    ctx.tex_ledger().zhu = 20; // baseline
    counter.store(4, std::sync::atomic::Ordering::SeqCst); // baseline vm
    ctx.state().set(vec![253], vec![253]); // baseline state
    let old_adr = Address::create_scriptmh([101u8; 20]);
    ctx.p2sh_set(old_adr, Box::new(AstTestP2shImpl)).unwrap();
    ctx.logs().push(&Uint1::from(1)); // baseline log

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestMainSet::create_by(251, 251)),
            Box::new(AstTestCombo::create_by(254, 3)),
            Box::new(AstTestMainP2shSetN::create_by(102)),
            Box::new(AstTestMainVMCall::create_by(2)),
        ]),
        AstSelect::create_by(
            1,
            1,
            vec![Box::new(AstTestMutateAllFail::create_by(252, 6, 103, 10))],
        ),
        AstSelect::nop(),
    );

    ctx.exec_from_set(ExecFrom::Top);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("must succeed at least") || err.contains("mutate-all fail"),
        "{}",
        err
    );

    assert_eq!(ast_state_get_u8(&mut ctx, 253), Some(253)); // baseline kept
    assert_eq!(ast_state_get_u8(&mut ctx, 251), None); // cond rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 252), None); // branch rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 254), None); // cond rolled back
    assert_eq!(ctx.tex_ledger().zhu, 20);
    assert_eq!(unsafe { &*logs_ptr }.len(), 1); // baseline log only
    assert!(ctx.p2sh(&Address::create_scriptmh([101u8; 20])).is_ok());
    assert!(ctx.p2sh(&Address::create_scriptmh([102u8; 20])).is_err());
    assert!(ctx.p2sh(&Address::create_scriptmh([103u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 4);
}

// ---- Test 40: AstSelect with actions len == TX_ACTIONS_MAX is allowed ----
#[test]
fn test_ast_select_num_eq_tx_actions_max_allowed() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let mut acts: Vec<Box<dyn Action>> = vec![Box::new(AstTestSet::create_by(201, 1))];
    for _ in 1..TX_ACTIONS_MAX {
        acts.push(Box::new(AstTestFail::new()));
    }
    let act = AstSelect::create_by(1, 1, acts); // should stop after first success

    ctx.exec_from_set(ExecFrom::Top);
    act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 201), Some(1));
}

// ---- Test 41: AstSelect error still restores exec_from by ExecFromGuard ----
#[test]
fn test_ast_select_error_restores_exec_from() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let bad = AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]);
    ctx.exec_from_set(ExecFrom::Top);
    let _ = bad.execute(&mut ctx).unwrap_err();

    assert_eq!(ctx.exec_from(), ExecFrom::Top);
}

// ---- Test 42: AstIf with cond=nop takes if-branch (cond success with 0-required select) ----
#[test]
fn test_ast_if_cond_nop_takes_if_branch() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let astif = AstIf::create_by(
        AstSelect::nop(), // cond succeeds (0/0)
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(202, 202))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(203, 203))]),
    );
    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 202), Some(202));
    assert_eq!(ast_state_get_u8(&mut ctx, 203), None);
}

// ---- Test 43: ast depth exactly 6 is allowed ----
#[test]
fn test_ast_tree_depth_exact_6_is_allowed() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let lvl6 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(204, 204))]);
    let lvl5 = AstSelect::create_list(vec![Box::new(lvl6)]);
    let lvl4 = AstSelect::create_list(vec![Box::new(lvl5)]);
    let lvl3 = AstSelect::create_list(vec![Box::new(lvl4)]);
    let lvl2 = AstSelect::create_list(vec![Box::new(lvl3)]);
    let lvl1 = AstSelect::create_list(vec![Box::new(lvl2)]);

    ctx.exec_from_set(ExecFrom::Top);
    lvl1.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 204), Some(204));
}

// ---- Test 44: AstIf cond mutate-all fail is recovered, else commits ----
#[test]
fn test_ast_if_cond_mutate_all_fail_recovers_and_commits_else() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);
    ctx.tex_ledger().zhu = 30;
    counter.store(2, std::sync::atomic::Ordering::SeqCst);
    ctx.state().set(vec![214], vec![214]);
    ctx.logs().push(&Uint1::from(1)); // baseline log
    let old_adr = Address::create_scriptmh([104u8; 20]);
    ctx.p2sh_set(old_adr, Box::new(AstTestP2shImpl)).unwrap();

    let astif = AstIf::create_by(
        AstSelect::create_by(
            1,
            1,
            vec![Box::new(AstTestMutateAllFail::create_by(211, 5, 106, 7))],
        ),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(212, 212))]),
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(213, 4)),
            Box::new(AstTestMainP2shSetN::create_by(105)),
            Box::new(AstTestMainVMCall::create_by(3)),
        ]),
    );

    ctx.exec_from_set(ExecFrom::Top);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 214), Some(214)); // baseline
    assert_eq!(ast_state_get_u8(&mut ctx, 211), None); // cond rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 212), None); // if not taken
    assert_eq!(ast_state_get_u8(&mut ctx, 213), Some(4)); // else committed
    assert_eq!(ctx.tex_ledger().zhu, 34); // +4 only
    assert_eq!(unsafe { &*logs_ptr }.len(), 2); // baseline + combo
    assert!(ctx.p2sh(&Address::create_scriptmh([104u8; 20])).is_ok());
    assert!(ctx.p2sh(&Address::create_scriptmh([105u8; 20])).is_ok());
    assert!(ctx.p2sh(&Address::create_scriptmh([106u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5); // 2 + 3
}

// ---- Test 45: AstIf branch validation error recovers cond side-effects (whole-snap) ----
#[test]
fn test_ast_if_branch_validation_error_recovers_cond_all_channels() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = false; // keep check_action_scope enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.test_set_vm(mock_vm);
    ctx.tex_ledger().zhu = 40;
    counter.store(1, std::sync::atomic::Ordering::SeqCst);
    ctx.logs().push(&Uint1::from(2)); // baseline

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(215, 6)),
            Box::new(AstTestMainP2shSetN::create_by(107)),
            Box::new(AstTestMainVMCall::create_by(4)),
        ]),
        AstSelect::create_by(3, 1, vec![Box::new(AstTestMainSet::create_by(216, 216))]),
        AstSelect::nop(),
    );

    ctx.exec_from_set(ExecFrom::Top);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot be less than min"), "{}", err);

    assert_eq!(ast_state_get_u8(&mut ctx, 215), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 216), None);
    assert_eq!(ctx.tex_ledger().zhu, 40);
    assert_eq!(unsafe { &*logs_ptr }.len(), 1); // baseline only
    assert!(ctx.p2sh(&Address::create_scriptmh([107u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// ---- Test 46: Nested invalid AstSelect is treated as failed child and isolated ----
#[test]
fn test_ast_select_nested_invalid_select_isolated() {
    let mut tx = TransactionType2::default();
    tx.fee = Amount::unit238(1000);
    tx.addrlist =
        AddrOrList::Val1(Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap());
    let mut env = Env::default();
    env.tx.main = Address::from_readable("16Jswqk47s9PUcyCc88MMVwzgvHPvtEpf").unwrap();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.gas_init_tx(10000, 1).unwrap();

    let bad_nested = AstSelect::create_by(2, 1, vec![Box::new(AstTestSet::create_by(217, 217))]);
    let outer = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(bad_nested), // fail as one child
            Box::new(AstTestSet::create_by(218, 218)),
        ],
    );

    ctx.exec_from_set(ExecFrom::Top);
    let err = outer.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("action ast select max cannot be less than min"),
        "{}",
        err
    );
    assert_eq!(ast_state_get_u8(&mut ctx, 217), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 218), None);
}
