use crate::action::*;
use crate::block::*;
use crate::transaction::*;
use basis::component::*;
use basis::interface::*;
use field::*;
use std::sync::Once;
use sys::*;

static INIT: Once = Once::new();
static INIT_EXT_ENV: Once = Once::new();

fn init_test_registry() {
    INIT.call_once(|| {
        crate::setup::action_register(crate::action::try_create, crate::action::try_json_decode);
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
        let pairs = json_split_object(json_str);
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
    fn execute(&self, _ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
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

fn ext_env_try_create(kind: u16, buf: &[u8]) -> Ret<Option<(Box<dyn Action>, usize)>> {
    if kind != TestExtEnvReadOnly::KIND {
        return Ok(None);
    }
    let (act, sk) = TestExtEnvReadOnly::create(buf)?;
    Ok(Some((Box::new(act), sk)))
}

fn ext_env_try_json_decode(kind: u16, json: &str) -> Ret<Option<Box<dyn Action>>> {
    if kind != TestExtEnvReadOnly::KIND {
        return Ok(None);
    }
    let mut act = TestExtEnvReadOnly::default();
    act.from_json(json)?;
    Ok(Some(Box::new(act)))
}

fn init_ext_env_test_registry() {
    INIT_EXT_ENV.call_once(|| {
        crate::setup::action_register(ext_env_try_create, ext_env_try_json_decode);
    });
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
    let tx = TransactionType2::default();

    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];

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
    env.tx.main = tx.main();
    env.tx.addrs = tx.addrs();
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
fn test_ctx_action_call_extenv_does_not_require_tx_main_signature() {
    init_test_registry();
    init_ext_env_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::new_by(field::ADDRESS_ONEX.clone(), Amount::mei(1), 1730000000);

    let mut env = Env::default();
    env.tx.main = tx.main();
    env.tx.addrs = tx.addrs();

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
    let tx = TransactionType2::default();

    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone(), field::ADDRESS_TWOX.clone()];

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
fn test_ctx_action_call_must_reject_trailing_bytes() {
    init_test_registry();

    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
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

#[test]
fn test_address_bare_base58check_in_protocol() {
    // Verify Address can parse from JSON with bare base58check (no b58: prefix)
    let addr_str = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
    let mut addr = Address::default();
    addr.from_json(&format!(r#""{}""#, addr_str)).unwrap();
    assert_eq!(addr.to_readable(), addr_str);

    // Verify AddrOrPtr with Address parses bare base58check
    let mut ptr = AddrOrPtr::default();
    ptr.from_json(&format!(r#"{{"type":1,"value":"{}"}}"#, addr_str))
        .unwrap();
    assert_eq!(ptr.to_readable(), addr_str);
}

#[cfg(feature = "ast")]
#[derive(Default, Clone)]
struct AstTestState {
    parent: std::sync::Weak<Box<dyn State>>,
    mem: MemMap,
}

#[cfg(feature = "ast")]
impl State for AstTestState {
    fn fork_sub(&self, p: std::sync::Weak<Box<dyn State>>) -> Box<dyn State> {
        Box::new(Self {
            parent: p,
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

#[cfg(feature = "ast")]
fn build_ast_ctx_with_state<'a>(
    env: Env,
    sta: Box<dyn State>,
    tx: &'a dyn TransactionRead,
) -> crate::context::ContextInst<'a> {
    use crate::state::EmptyLogs;
    crate::context::ContextInst::new(env, sta, Box::new(EmptyLogs {}), tx)
}

#[cfg(feature = "ast")]
fn ast_state_get_u8(ctx: &mut dyn Context, key: u8) -> Option<u8> {
    ctx.state().get(vec![key]).and_then(|v| v.first().copied())
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestSet {
    key: Uint1,
    val: Uint1,
}

#[cfg(feature = "ast")]
impl Parse for AstTestSet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}

#[cfg(feature = "ast")]
impl Serialize for AstTestSet {
    fn serialize(&self) -> Vec<u8> {
        [self.key.serialize(), self.val.serialize()].concat()
    }

    fn size(&self) -> usize {
        self.key.size() + self.val.size()
    }
}

#[cfg(feature = "ast")]
impl Field for AstTestSet {
    fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "ast")]
impl ToJSON for AstTestSet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}

#[cfg(feature = "ast")]
impl FromJSON for AstTestSet {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
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

#[cfg(feature = "ast")]
impl ActExec for AstTestSet {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        Ok((0, vec![]))
    }
}

#[cfg(feature = "ast")]
impl Description for AstTestSet {}

#[cfg(feature = "ast")]
impl Action for AstTestSet {
    fn kind(&self) -> u16 {
        65001
    }
    fn level(&self) -> ActLv {
        ActLv::Ast
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(feature = "ast")]
impl AstTestSet {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
            ..Self::new()
        }
    }
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestFail {}

#[cfg(feature = "ast")]
impl Parse for AstTestFail {
    fn parse(&mut self, _buf: &[u8]) -> Ret<usize> {
        Ok(0)
    }
}

#[cfg(feature = "ast")]
impl Serialize for AstTestFail {
    fn serialize(&self) -> Vec<u8> {
        vec![]
    }

    fn size(&self) -> usize {
        0
    }
}

#[cfg(feature = "ast")]
impl Field for AstTestFail {
    fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "ast")]
impl ToJSON for AstTestFail {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        "{}".to_owned()
    }
}

#[cfg(feature = "ast")]
impl FromJSON for AstTestFail {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        Ok(())
    }
}

#[cfg(feature = "ast")]
impl ActExec for AstTestFail {
    fn execute(&self, _ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        errf!("ast test forced fail")
    }
}

#[cfg(feature = "ast")]
impl Description for AstTestFail {}

#[cfg(feature = "ast")]
impl Action for AstTestFail {
    fn kind(&self) -> u16 {
        65002
    }
    fn level(&self) -> ActLv {
        ActLv::Ast
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_if_cond_true_commits_cond_and_if_branch_state() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true; // keep focus on AST semantics
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let cond = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(1, 11))]);
    let br_if = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(2, 22))]);
    let br_else = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(3, 33))]);
    let astif = AstIf::create_by(cond, br_if, br_else);

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(11)); // cond committed
    assert_eq!(ast_state_get_u8(&mut ctx, 2), Some(22)); // if branch committed
    assert_eq!(ast_state_get_u8(&mut ctx, 3), None); // else branch not executed
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_select_partial_write_is_reverted_by_tx_level_rollback() {
    let mut tx = TransactionType2::default();
    tx.ty = Uint1::from(TransactionType2::TYPE);
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
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![9], vec![99]); // parent baseline

    let old = ctx.state_fork(); // tx-level isolation
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));
    ctx.state_recover(old); // tx-level rollback on failure

    assert_eq!(ast_state_get_u8(&mut ctx, 9), Some(99)); // baseline kept
    assert_eq!(ast_state_get_u8(&mut ctx, 7), None); // child write rolled back
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_nested_if_select_else_path_commits_expected_layers() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    outer_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 50), Some(50)); // outer cond
    assert_eq!(ast_state_get_u8(&mut ctx, 51), Some(51)); // outer if branch
    assert_eq!(ast_state_get_u8(&mut ctx, 53), Some(53)); // inner else branch
    assert_eq!(ast_state_get_u8(&mut ctx, 52), None); // inner if branch not executed
    assert_eq!(ast_state_get_u8(&mut ctx, 54), None); // outer else not executed
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_nested_select_failure_does_not_leak_into_outer_select() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    outer.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 60), Some(60));
    assert_eq!(ast_state_get_u8(&mut ctx, 62), Some(62));
    assert_eq!(ast_state_get_u8(&mut ctx, 61), None); // nested failed select write must not leak
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_nested_partial_commits_are_cleared_by_tx_level_rollback() {
    let mut tx = TransactionType2::default();
    tx.ty = Uint1::from(TransactionType2::TYPE);

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
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![79], vec![79]); // baseline

    let old = ctx.state_fork();
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = tx.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));
    ctx.state_recover(old);

    assert_eq!(ast_state_get_u8(&mut ctx, 79), Some(79)); // baseline kept
    assert_eq!(ast_state_get_u8(&mut ctx, 70), None); // nested partial commit must be rolled back at tx level
    assert_eq!(ast_state_get_u8(&mut ctx, 71), None); // nested partial commit must be rolled back at tx level
    assert_eq!(ast_state_get_u8(&mut ctx, 72), None);
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_deep_4level_success_path_commits_expected_state() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    lvl1_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 80), Some(80));
    assert_eq!(ast_state_get_u8(&mut ctx, 81), Some(81));
    assert_eq!(ast_state_get_u8(&mut ctx, 82), Some(82));
    assert_eq!(ast_state_get_u8(&mut ctx, 83), Some(83));
    assert_eq!(ast_state_get_u8(&mut ctx, 84), Some(84));
    assert_eq!(ast_state_get_u8(&mut ctx, 88), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 89), None);
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_deep_4level_failed_branch_isolated_by_outer_select() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    lvl1_outer_select.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 90), Some(90));
    assert_eq!(ast_state_get_u8(&mut ctx, 95), Some(95));
    assert_eq!(ast_state_get_u8(&mut ctx, 91), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 92), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 93), None); // must be isolated
    assert_eq!(ast_state_get_u8(&mut ctx, 94), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 96), None);
}

#[cfg(feature = "ast")]
#[test]
fn test_ast_tree_depth_limit_6_rejects_7th_level() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let lvl7 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(105, 105))]);
    let lvl6 = AstSelect::create_list(vec![Box::new(lvl7)]);
    let lvl5 = AstSelect::create_list(vec![Box::new(lvl6)]);
    let lvl4 = AstSelect::create_list(vec![Box::new(lvl5)]);
    let lvl3 = AstSelect::create_list(vec![Box::new(lvl4)]);
    let lvl2 = AstSelect::create_list(vec![Box::new(lvl3)]);
    let lvl1 = AstSelect::create_list(vec![Box::new(lvl2)]);

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = lvl1.execute(&mut ctx).unwrap_err();
    assert!(
        err.contains("must succeed at least 1 but only 0"),
        "{}",
        err
    );
    assert_eq!(ast_state_get_u8(&mut ctx, 105), None);
}

#[cfg(feature = "ast")]
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
        fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
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
        fn level(&self) -> ActLv {
            ActLv::Ast
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    use crate::state::EmptyLogs;
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = crate::context::ContextInst::new(
        env,
        Box::new(AstTestState::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
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
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();
    assert_eq!(ctx.tex_ledger().zhu, old_tex.zhu);
    assert_eq!(ctx.tex_ledger().sat, old_tex.sat);
    assert!(ctx.p2sh(&old_adr).is_ok());
    assert!(ctx.p2sh(&new_adr).is_err());
}

#[cfg(feature = "tex")]
#[derive(Default)]
struct TestMemState {
    kv: std::collections::HashMap<Vec<u8>, Vec<u8>>,
}

#[cfg(feature = "tex")]
impl State for TestMemState {
    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        self.kv.get(&k).cloned()
    }
    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.kv.insert(k, v);
    }
    fn del(&mut self, k: Vec<u8>) {
        self.kv.remove(&k);
    }
}

#[cfg(feature = "tex")]
fn build_tex_ctx_with_state(env: Env, sta: Box<dyn State>) -> crate::context::ContextInst<'static> {
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;
    let tx = Box::leak(Box::new(TransactionType2::default()));
    crate::context::ContextInst::new(env, sta, Box::new(EmptyLogs {}), tx)
}

#[cfg(feature = "tex")]
#[test]
fn test_tex_sat_pay_records_sat_not_zhu() {
    use crate::tex::*;

    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    {
        let mut st = crate::state::CoreState::wrap(ctx.state());
        let mut bls = Balance::default();
        bls.satoshi = Fold64::from(100).unwrap();
        st.balance_set(&addr, &bls);
    }

    let cell = CellTrsSatPay::new(Fold64::from(7).unwrap());
    cell.execute(&mut ctx, &addr).unwrap();

    assert_eq!(ctx.tex_ledger().sat, 7);
    assert_eq!(ctx.tex_ledger().zhu, 0);
}

#[cfg(feature = "tex")]
#[test]
fn test_tex_asset_serial_must_exist_and_cache() {
    use crate::tex::*;

    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    {
        let mut st = crate::state::CoreState::wrap(ctx.state());
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
    assert!(miss.contains("not exist"));

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

#[cfg(feature = "tex")]
#[test]
fn test_tex_diamond_get_zero_rejected_early() {
    use crate::tex::*;

    let mut env = Env::default();
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let addr = field::ADDRESS_ONEX.clone();

    let mut ctx = build_tex_ctx_with_state(env, Box::new(TestMemState::default()));
    let err = CellTrsDiaGet::new(DiamondNumber::from(0))
        .execute(&mut ctx, &addr)
        .unwrap_err();
    assert!(err.contains("cannot be zero"));
}

#[cfg(feature = "tex")]
#[test]
fn test_tex_cell_json_must_use_cellid() {
    use crate::tex::*;

    let mut ls = DnyTexCellW1::default();
    let ok_json = r#"[{"cellid":11,"haczhu":0}]"#;
    ls.from_json(ok_json).unwrap();
    assert_eq!(ls.length(), 1);

    let mut bad = DnyTexCellW1::default();
    let err = bad.from_json(r#"[{"kind":11,"haczhu":0}]"#).unwrap_err();
    assert!(err.contains("cellid"));
}

#[cfg(feature = "tex")]
#[test]
fn test_tex_action_signature_rejects_payload_tamper() {
    use crate::tex::*;

    let mut env = Env::default();
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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("signature verify failed"));
}
