use crate::action::*;
use crate::block::*;
use crate::transaction::*;
use basis::component::*;
use basis::interface::*;
use field::*;
use std::sync::Once;
use sys::*;

static INIT: Once = Once::new();

fn init_test_registry() {
    INIT.call_once(|| {
        let builder = crate::setup::SetupBuilder::new()
            .block_hasher(|_, stuff| sys::calculate_hash(stuff))
            .action_register(crate::action::register);
        let registry = builder
            .register_codec(
                &[TestExtEnvReadOnly::KIND],
                action_env_try_create,
                action_env_try_json_decode,
                false,
            )
            .build()
            .unwrap();
        crate::setup::install_once(registry).unwrap();
    });
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct TestExtEnvReadOnly {
    kind: Uint2,
}

impl TestExtEnvReadOnly {
    const KIND: u16 = 0x07f0;
}

impl Parse for TestExtEnvReadOnly {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.kind.parse(buf)
    }
}

impl Serialize for TestExtEnvReadOnly {
    fn serialize(&self) -> Vec<u8> {
        self.kind.serialize()
    }
    fn size(&self) -> usize {
        self.kind.size()
    }
}

impl Field for TestExtEnvReadOnly {
    fn new() -> Self {
        Self {
            kind: Uint2::from(Self::KIND),
        }
    }
}

impl ToJSON for TestExtEnvReadOnly {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(r#"{{"kind":{}}}"#, self.kind.to_json_fmt(fmt))
    }
}

impl FromJSON for TestExtEnvReadOnly {
    fn from_json(&mut self, json_str: &str) -> Ret<()> {
        let pairs = json_split_object(json_str)?;
        for (k, v) in pairs {
            if k == "kind" {
                self.kind.from_json(v)?;
            }
        }
        Ok(())
    }
}

impl Description for TestExtEnvReadOnly {
    fn to_description(&self) -> String {
        "Test ext env read only action".to_owned()
    }
}

impl ActExec for TestExtEnvReadOnly {
    fn execute(&self, _ctx: &mut dyn Context) -> XRet<(u32, Vec<u8>)> {
        Ok((0, vec![1u8]))
    }
}

impl Action for TestExtEnvReadOnly {
    fn kind(&self) -> u16 {
        *self.kind
    }
    fn level(&self) -> ActLv {
        ActLv::Any
    }
    fn req_sign(&self) -> Vec<AddrOrPtr> {
        vec![]
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn action_env_try_create(kind: u16, buf: &[u8]) -> Ret<Option<(Box<dyn Action>, usize)>> {
    if kind != TestExtEnvReadOnly::KIND {
        return Ok(None);
    }
    let (act, sk) = TestExtEnvReadOnly::create(buf)?;
    Ok(Some((Box::new(act), sk)))
}

fn action_env_try_json_decode(kind: u16, json: &str) -> Ret<Option<Box<dyn Action>>> {
    if kind != TestExtEnvReadOnly::KIND {
        return Ok(None);
    }
    let mut act = TestExtEnvReadOnly::default();
    act.from_json(json)?;
    Ok(Some(Box::new(act)))
}

fn init_action_env_test_registry() {
    init_test_registry();
}

#[derive(Default, Clone)]
struct AstForkableState {
    parent: std::sync::Weak<Box<dyn State>>,
    mem: MemMap,
}

impl State for AstForkableState {
    fn fork_sub(&self, parent: std::sync::Weak<Box<dyn State>>) -> Box<dyn State> {
        Box::new(Self {
            parent,
            mem: MemMap::default(),
        })
    }

    fn merge_sub(&mut self, sta: Box<dyn State>) {
        self.mem.extend(sta.as_mem().clone());
    }

    fn detach(&mut self) {
        self.parent = std::sync::Weak::<Box<dyn State>>::new();
    }

    fn clone_state(&self) -> Box<dyn State> {
        Box::new(self.clone())
    }

    fn as_mem(&self) -> &MemMap {
        &self.mem
    }

    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        if let Some(v) = self.mem.get(&k) {
            return v.clone();
        }
        if let Some(parent) = self.parent.upgrade() {
            return parent.get(k);
        }
        None
    }

    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.mem.insert(k, Some(v));
    }

    fn del(&mut self, k: Vec<u8>) {
        self.mem.insert(k, None);
    }
}

#[test]
fn test_transaction_json_full_cycle() {
    init_test_registry();

    // 1. Create a TransactionType2 with some actions
    let mut tx = TransactionType2::default();
    tx.ty = Uint1::from(TransactionType2::TYPE);
    tx.timestamp = Timestamp::from(1730000000);
    tx.fee = Amount::small(1, 244); // 1.0 HAC

    // Add an action: HacToTrs
    let mut act1 = HacToTrs::new();
    act1.to = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    act1.hacash = Amount::small(5, 244); // 5.0 HAC
    tx.actions.push(Box::new(act1)).unwrap();

    // Add another action: DiaToTrs
    let mut act2 = DiaToTrs::new();
    act2.to = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    act2.diamonds = DiamondNameListMax200::from_readable("WTYUIA").unwrap();
    tx.actions.push(Box::new(act2)).unwrap();

    // Add another action: AssetToTrs
    let mut act3 = AssetToTrs::new();
    act3.to = AddrOrPtr::from_ptr(1);
    act3.asset = AssetAmt::from(100, 500).unwrap();
    tx.actions.push(Box::new(act3)).unwrap();

    // 2. Serialize to Binary
    let bin1 = tx.serialize();

    // 3. Serialize to JSON
    let json = tx.to_json();
    println!("Transaction JSON: {}", json);

    // 4. Deserialize from JSON
    let mut tx2 = TransactionType2::default();
    tx2.from_json(&json).expect("JSON Deserialization failed");

    // 5. Serialize reconstructed to Binary
    let bin2 = tx2.serialize();

    // 6. Compare
    assert_eq!(
        bin1, bin2,
        "Binary mismatch after Transaction JSON round-trip"
    );
    assert_eq!(*tx2.timestamp, 1730000000);
    assert_eq!(tx2.actions.length(), 3);
}

#[test]
fn test_block_json_full_cycle() {
    init_test_registry();

    // 1. Create a BlockV1
    let mut block = BlockV1::default();
    block.intro.head.height = BlockHeight::from(100);
    block.intro.head.timestamp = Timestamp::from(1730000000);
    block.intro.head.prevhash = Hash::from([1u8; 32]);

    // Create and add Transaction 1
    let mut tx1 = TransactionType2::default();
    tx1.ty = Uint1::from(TransactionType2::TYPE);
    tx1.timestamp = Timestamp::from(1730000001);
    tx1.fee = Amount::mei(100);
    let mut act1 = HacToTrs::new();
    act1.to = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    act1.hacash = Amount::small(1, 244);
    tx1.actions.push(Box::new(act1)).unwrap();

    block.transactions.push(Box::new(tx1)).unwrap();

    // Create and add Transaction 2 (Coinbase)
    let mut tx2 = TransactionCoinbase::default();
    tx2.ty = Uint1::from(0); // Coinbase is usually 0
    tx2.reward = Amount::small(1, 248); // 1.0 HAC
    tx2.address = field::ADDRESS_TWOX.clone();
    let msg = "hello hacash".as_bytes();
    let mut msg_fixed = [0u8; 16];
    msg_fixed[..msg.len()].copy_from_slice(msg);
    tx2.message = Fixed16::from(msg_fixed);

    block.transactions.push(Box::new(tx2)).unwrap();

    // Update transaction count in header
    block.intro.head.transaction_count = Uint4::from(block.transactions.length() as u32);

    // 2. Serialize to Binary
    let bin1 = block.serialize();

    // 3. Serialize to JSON
    let json = block.to_json();
    println!("Block JSON: {}", json);

    // 4. Deserialize from JSON
    let mut block2 = BlockV1::default();
    block2
        .from_json(&json)
        .expect("Block JSON Deserialization failed");

    // 5. Serialize reconstructed to Binary
    let bin2 = block2.serialize();

    // 6. Compare
    assert_eq!(bin1, bin2, "Binary mismatch after Block JSON round-trip");
    assert_eq!(block2.transactions.length(), 2);
    assert_eq!(*block2.intro.head.height, 100);
}

#[test]
fn test_ctx_action_call_must_check_req_sign() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    // tx without any signatures
    let tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);

    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);

    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    // HacFromTrs requires signature of `from`, but since this action is executed via ctx.action_call,
    // it would bypass tx.req_sign unless ctx_action_call enforces it.
    let mut act = HacFromTrs::new();
    act.from = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    act.hacash = Amount::mei(1);
    let bytes = act.serialize();

    let err = ctx
        .action_call(HacFromTrs::KIND, bytes[2..].to_vec())
        .unwrap_err();
    assert!(err.contains("signature") || err.contains("failed") || err.contains("verify"));
}

#[test]
fn test_tx_execute_must_verify_signature_before_actions() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let mut tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    let mut act = HacToTrs::new();
    act.to = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    act.hacash = Amount::mei(1);
    tx.actions.push(Box::new(act)).unwrap();

    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("signature") || err.contains("failed") || err.contains("verify"),
        "{}",
        err
    );
}

#[test]
fn test_tx_execute_must_reject_action_count_over_max() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let mut tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    for _ in 0..(TX_ACTIONS_MAX + 1) {
        let mut act = HacToTrs::new();
        act.to = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
        act.hacash = Amount::mei(1);
        tx.actions.push(Box::new(act)).unwrap();
    }

    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("one transaction max actions"), "{}", err);
}

#[test]
fn test_tx_req_sign_must_be_privakey_address() {
    init_test_registry();

    use crate::transaction::TransactionType2;

    let mut tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    // scriptmh address (non-privakey, cannot sign) - should be ignored by tx.req_sign()
    let scriptmh_addr = Address::create_scriptmh([1u8; 20]);

    let mut act = HacFromTrs::new();
    act.from = AddrOrPtr::from_addr(scriptmh_addr);
    act.hacash = Amount::mei(1);
    tx.actions.push(Box::new(act)).unwrap();

    let signset = tx.req_sign().unwrap();
    assert!(signset.contains(&tx.main()));
    assert!(!signset.contains(&scriptmh_addr));
}

#[test]
fn test_ctx_action_call_actenv_does_not_require_tx_main_signature() {
    init_test_registry();
    init_action_env_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::default();

    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);

    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let res = ctx.action_call(TestExtEnvReadOnly::KIND, vec![]).unwrap();
    assert_eq!(res.1, vec![1u8]);
}

#[test]
fn test_tx_req_sign_must_collect_nested_ast_child_actions() {
    init_test_registry();

    use crate::transaction::TransactionType2;

    let mut tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    let nested_signer = field::ADDRESS_TWOX.clone();

    let mut leaf = HacFromTrs::new();
    leaf.from = AddrOrPtr::from_addr(nested_signer);
    leaf.hacash = Amount::mei(1);

    let nested_if = AstIf::create_by(
        AstSelect::nop(),
        AstSelect::create_list(vec![Box::new(AstSelect::create_list(vec![Box::new(leaf)]))]),
        AstSelect::nop(),
    );
    tx.actions
        .push(Box::new(AstSelect::create_list(vec![Box::new(nested_if)])))
        .unwrap();

    let signset = tx.req_sign().unwrap();
    assert!(signset.contains(&tx.main()));
    assert!(signset.contains(&nested_signer));
}

#[test]
fn test_ctx_action_call_must_check_nested_ast_req_sign() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    // tx without any signatures
    let tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);

    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);

    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let mut leaf = HacFromTrs::new();
    leaf.from = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    leaf.hacash = Amount::mei(1);
    let act = AstSelect::create_list(vec![Box::new(leaf)]);
    let bytes = act.serialize();

    let err = ctx
        .action_call(AstSelect::KIND, bytes[2..].to_vec())
        .unwrap_err();
    assert!(
        err.contains("signature") || err.contains("failed") || err.contains("verify"),
        "{}",
        err
    );
}

#[test]
fn test_tx_req_sign_astif_must_collect_cond_if_else_and_filter_scriptmh() {
    init_test_registry();

    use crate::transaction::TransactionType2;

    let mut tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    let scriptmh = Address::create_scriptmh([9u8; 20]);

    let mut cond = HacFromTrs::new();
    cond.from = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    cond.hacash = Amount::mei(1);

    let mut br_if = HacFromTrs::new();
    br_if.from = AddrOrPtr::from_addr(scriptmh);
    br_if.hacash = Amount::mei(1);

    let mut br_else = HacFromTrs::new();
    br_else.from = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    br_else.hacash = Amount::mei(1);

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(cond)]),
        AstSelect::create_list(vec![Box::new(br_if)]),
        AstSelect::create_list(vec![Box::new(br_else)]),
    );
    tx.actions.push(Box::new(astif)).unwrap();

    let signset = tx.req_sign().unwrap();
    assert!(signset.contains(&field::ADDRESS_ONEX.clone()));
    assert!(signset.contains(&field::ADDRESS_TWOX.clone()));
    assert!(!signset.contains(&scriptmh)); // scriptmh cannot sign
    assert_eq!(signset.len(), 2);
}

#[test]
fn test_ctx_action_call_astif_must_check_unreachable_branch_req_sign() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    // tx without any signatures
    let tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);
    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);

    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    // cond=nop makes else logically unreachable, but req_sign is static and must still be checked.
    let mut leaf = HacFromTrs::new();
    leaf.from = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    leaf.hacash = Amount::mei(1);
    let act = AstIf::create_by(
        AstSelect::nop(),
        AstSelect::nop(),
        AstSelect::create_list(vec![Box::new(leaf)]),
    );
    let bytes = act.serialize();

    let err = ctx
        .action_call(AstIf::KIND, bytes[2..].to_vec())
        .unwrap_err();
    assert!(
        err.contains("signature") || err.contains("failed") || err.contains("verify"),
        "{}",
        err
    );
}

#[test]
fn test_ast_select_min_failure_is_revert() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(AstForkableState::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
    {
        let main = field::ADDRESS_ONEX.clone();
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&main).unwrap_or_default();
        bls.hacash = Amount::unit238(1_000_000_000);
        state.balance_set(&main, &bls);
    }
    ctx.gas_init_tx(10_000, 1).unwrap();

    let mut bad_guard = HeightScope::new();
    bad_guard.start = BlockHeight::from(10);
    bad_guard.end = BlockHeight::from(20);
    let act = AstSelect::create_by(1, 1, vec![Box::new(bad_guard)]);
    let bytes = act.serialize();

    let err = ctx
        .action_call(AstSelect::KIND, bytes[2..].to_vec())
        .unwrap_err();
    assert!(err.is_revert(), "{}", err);
    assert!(err.contains("must succeed at least"), "{}", err);
}

#[test]
fn test_ast_if_rethrow_preserves_revert_kind() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(AstForkableState::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
    {
        let main = field::ADDRESS_ONEX.clone();
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&main).unwrap_or_default();
        bls.hacash = Amount::unit238(1_000_000_000);
        state.balance_set(&main, &bls);
    }
    ctx.gas_init_tx(10_000, 1).unwrap();

    let mut cond_guard = HeightScope::new();
    cond_guard.start = BlockHeight::from(20);
    cond_guard.end = BlockHeight::from(30);
    let cond = AstSelect::create_by(1, 1, vec![Box::new(cond_guard)]);

    let mut else_guard = HeightScope::new();
    else_guard.start = BlockHeight::from(30);
    else_guard.end = BlockHeight::from(40);
    let br_else = AstSelect::create_by(1, 1, vec![Box::new(else_guard)]);

    let act = AstIf::create_by(cond, AstSelect::nop(), br_else);
    let bytes = act.serialize();

    let err = ctx
        .action_call(AstIf::KIND, bytes[2..].to_vec())
        .unwrap_err();
    assert!(err.is_revert(), "{}", err);
}

#[test]
fn test_balance_floor_empty_and_duplicate_asset_rejected() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(AstForkableState::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let mut empty = BalanceFloor::new();
    empty.addr = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    let err = ctx
        .action_call(BalanceFloor::KIND, empty.serialize()[2..].to_vec())
        .unwrap_err();
    assert!(err.contains("is empty"), "{}", err);

    let mut dup = BalanceFloor::new();
    dup.addr = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    dup.hacash = Amount::mei(1);
    dup.assets
        .push(AssetAmt::from(9527u64, 1u64).unwrap())
        .unwrap();
    dup.assets
        .push(AssetAmt::from(9527u64, 2u64).unwrap())
        .unwrap();
    let err = ctx
        .action_call(BalanceFloor::KIND, dup.serialize()[2..].to_vec())
        .unwrap_err();
    assert!(err.contains("duplicate"), "{}", err);
}

#[test]
fn test_balance_floor_success_and_insufficient() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(AstForkableState::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
    {
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&field::ADDRESS_ONEX).unwrap_or_default();
        bls.hacash = Amount::mei(1000);
        bls.satoshi = SatoshiAuto::from_satoshi(&Satoshi::from(20u64));
        bls.diamond = DiamondNumberAuto::from_diamond(&DiamondNumber::from(3u32));
        bls.asset_set(AssetAmt::from(88u64, 9u64).unwrap()).unwrap();
        state.balance_set(&field::ADDRESS_ONEX, &bls);
    }

    let mut ok = BalanceFloor::new();
    ok.addr = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    ok.hacash = Amount::mei(900);
    ok.satoshi = Satoshi::from(10u64);
    ok.diamond = DiamondNumber::from(2u32);
    ok.assets
        .push(AssetAmt::from(88u64, 7u64).unwrap())
        .unwrap();
    let _ = ctx
        .action_call(BalanceFloor::KIND, ok.serialize()[2..].to_vec())
        .unwrap();

    let mut bad = BalanceFloor::new();
    bad.addr = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
    bad.assets
        .push(AssetAmt::from(88u64, 10u64).unwrap())
        .unwrap();
    let err = ctx
        .action_call(BalanceFloor::KIND, bad.serialize()[2..].to_vec())
        .unwrap_err();
    assert!(err.contains("lower than floor"), "{}", err);
}

#[test]
fn test_ctx_action_call_must_reject_trailing_bytes() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.tx = crate::transaction::create_tx_info(&tx);
    let mut ctx = ContextInst::new(
        env,
        Box::new(crate::context::EmptyState {}),
        Box::new(EmptyLogs {}),
        &tx,
    );

    let mut act = HacToTrs::new();
    act.to = AddrOrPtr::from_addr(field::ADDRESS_TWOX.clone());
    act.hacash = Amount::mei(1);
    let mut body = act.serialize()[2..].to_vec();
    body.push(0x00); // trailing garbage

    let err = ctx.action_call(HacToTrs::KIND, body).unwrap_err();
    assert!(err.contains("parse length mismatch"), "{}", err);
}

#[test]
fn test_action_json_create_must_reject_kind_mismatch() {
    init_test_registry();

    let json = r#"{"kind":14}"#;
    let err = crate::action::action_json_create(HacToTrs::KIND, json).unwrap_err();
    assert!(err.contains("kind mismatch"), "{}", err);
}

#[test]
fn test_complex_json_structure() {
    init_test_registry();

    // Test a very complex transaction with many actions
    let mut tx = TransactionType1::default();
    tx.ty = Uint1::from(TransactionType1::TYPE);
    tx.fee = Amount::small(1, 240);

    for i in 1..=20 {
        let mut act = HacToTrs::new();
        act.to = AddrOrPtr::from_addr(field::ADDRESS_ONEX.clone());
        act.hacash = Amount::mei(i as u64);
        tx.actions.push(Box::new(act)).unwrap();
    }

    let json = tx.to_json();
    let mut tx2 = TransactionType1::default();
    tx2.from_json(&json)
        .expect("Complex JSON Deserialization failed");

    assert_eq!(tx.serialize(), tx2.serialize());
    assert_eq!(tx2.actions.length(), 20);
}

#[test]
fn test_transaction_base58check_format_roundtrip() {
    init_test_registry();

    // Round-trip with Base58Check format (Address outputs bare, no b58: prefix)
    let mut tx = TransactionType2::default();
    tx.ty = Uint1::from(TransactionType2::TYPE);
    tx.timestamp = Timestamp::from(1730000000);
    tx.fee = Amount::small(1, 244);
    tx.addrlist = AddrOrList::from_addr(
        Address::from_readable("1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS").unwrap(),
    );

    let mut act = HacToTrs::new();
    act.to =
        AddrOrPtr::from_addr(Address::from_readable("1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS").unwrap());
    act.hacash = Amount::small(5, 244);
    tx.actions.push(Box::new(act)).unwrap();

    let fmt_58 = field::JSONFormater {
        unit: "HAC".to_string(),
        binary: field::JSONBinaryFormat::Base58Check,
    };
    let json = tx.to_json_fmt(&fmt_58);

    // Verify Address is output without b58: prefix
    assert!(json.contains("1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS"));
    assert!(!json.contains("b58:1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS"));

    let mut tx2 = TransactionType2::default();
    tx2.from_json(&json)
        .expect("Base58Check format JSON parse failed");
    assert_eq!(tx.serialize(), tx2.serialize());
}

fn build_tex_test_ctx<'a>(tx: &'a dyn TransactionRead) -> crate::context::ContextInst<'a> {
    use crate::context::ContextInst;
    use crate::state::EmptyLogs;

    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx = crate::transaction::create_tx_info(tx);
    ContextInst::new(
        env,
        Box::new(AstForkableState::default()),
        Box::new(EmptyLogs {}),
        tx,
    )
}

#[test]
fn test_tex_zhu_condition_rejects_fractional_hac_balance() {
    use crate::tex::*;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut ctx = build_tex_test_ctx(&tx);
    {
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&field::ADDRESS_ONEX).unwrap_or_default();
        let half = Amount::zhu(1).ratio_floor(1, 2).unwrap();
        bls.hacash = Amount::zhu(1).add_mode_u128(&half).unwrap();
        state.balance_set(&field::ADDRESS_ONEX, &bls);
    }

    let err = CellCondZhuEq::new(Fold64::from(1).unwrap())
        .execute(&mut ctx, &field::ADDRESS_ONEX)
        .unwrap_err();
    assert!(err.contains("zhu check failed"), "{}", err);
}

#[test]
fn test_tex_zhu_pay_rejects_fractional_hac_balance() {
    use crate::tex::*;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut ctx = build_tex_test_ctx(&tx);
    {
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&field::ADDRESS_ONEX).unwrap_or_default();
        let half = Amount::zhu(1).ratio_floor(1, 2).unwrap();
        bls.hacash = Amount::zhu(1).add_mode_u128(&half).unwrap();
        state.balance_set(&field::ADDRESS_ONEX, &bls);
    }

    let err = CellTrsZhuPay::new(Fold64::from(1).unwrap())
        .execute(&mut ctx, &field::ADDRESS_ONEX)
        .unwrap_err();
    assert!(err.contains("whole-zhu"), "{}", err);
}

#[test]
fn test_tex_zhu_get_rejects_balance_below_one_zhu() {
    use crate::tex::*;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut ctx = build_tex_test_ctx(&tx);
    {
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&field::ADDRESS_ONEX).unwrap_or_default();
        bls.hacash = Amount::zhu(1).ratio_floor(1, 2).unwrap();
        state.balance_set(&field::ADDRESS_ONEX, &bls);
    }

    let err = CellTrsZhuGet::new(Fold64::from(1).unwrap())
        .execute(&mut ctx, &field::ADDRESS_ONEX)
        .unwrap_err();
    assert!(err.contains("zero or at least 1 zhu"), "{}", err);
}

#[test]
fn test_tex_zhu_condition_accepts_exact_one_zhu() {
    use crate::tex::*;

    let tx = TransactionType2::new_by(
        field::ADDRESS_ONEX.clone(),
        Amount::unit238(1000),
        1730000000,
    );
    let mut ctx = build_tex_test_ctx(&tx);
    {
        let mut state = crate::state::CoreState::wrap(ctx.state());
        let mut bls = state.balance(&field::ADDRESS_ONEX).unwrap_or_default();
        bls.hacash = Amount::zhu(1);
        state.balance_set(&field::ADDRESS_ONEX, &bls);
    }

    CellCondZhuEq::new(Fold64::from(1).unwrap())
        .execute(&mut ctx, &field::ADDRESS_ONEX)
        .unwrap();
}

#[test]
fn test_address_bare_base58check_in_protocol() {
    // Verify Address can parse from JSON with bare base58check (no b58: prefix)
    let addr_str = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
    let mut addr = Address::default();
    addr.from_json(&format!(r#""{}""#, addr_str)).unwrap();
    assert_eq!(addr.to_readable(), addr_str);

    // Verify AddrOrPtr with Address parses bare base58check
    let mut ptr = AddrOrPtr::default();
    ptr.from_json(&format!(r#""{}""#, addr_str)).unwrap();
    assert_eq!(ptr.to_readable(), addr_str);
}
