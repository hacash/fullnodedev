use crate::action::*;
use crate::block::*;
use crate::transaction::*;
use basis::component::*;
use basis::interface::*;
use field::*;
#[cfg(feature = "ast")]
use std::any::Any;
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

#[cfg(feature = "ast")]
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
        fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
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
        fn level(&self) -> ActLv {
            ActLv::Ast
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.tx.main = field::ADDRESS_ONEX.clone();
    env.tx.addrs = vec![field::ADDRESS_ONEX.clone()];
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let new_adr = Address::create_scriptmh([8u8; 20]);
    let act = AstSelect::create_by(
        2,
        2,
        vec![
            Box::new(AstTestP2shSetOnly::new()),
            Box::new(AstTestFail::new()),
        ],
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));
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

// =====================================================================
// Comprehensive AST snapshot/restore edge-case tests
// =====================================================================

// --- Test helper: action that pushes a log entry ---
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestLog {
    tag: Uint1,
}

#[cfg(feature = "ast")]
impl Parse for AstTestLog {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.tag.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestLog {
    fn serialize(&self) -> Vec<u8> { self.tag.serialize() }
    fn size(&self) -> usize { self.tag.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestLog { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestLog {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"tag\":{}}}", self.tag.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestLog {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs { if k == "tag" { self.tag.from_json(v)?; } }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestLog {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.logs().push(&self.tag);
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestLog {}
#[cfg(feature = "ast")]
impl Action for AstTestLog {
    fn kind(&self) -> u16 { 65005 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestLog {
    fn create_by(tag: u8) -> Self { Self { tag: Uint1::from(tag) } }
}

// --- Test helper: action that modifies tex_ledger ---
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestTexAdd {
    zhu_add: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestTexAdd {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.zhu_add.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestTexAdd {
    fn serialize(&self) -> Vec<u8> { self.zhu_add.serialize() }
    fn size(&self) -> usize { self.zhu_add.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestTexAdd { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestTexAdd {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"zhu_add\":{}}}", self.zhu_add.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestTexAdd {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs { if k == "zhu_add" { self.zhu_add.from_json(v)?; } }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestTexAdd {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.tex_ledger().zhu += *self.zhu_add as i64;
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestTexAdd {}
#[cfg(feature = "ast")]
impl Action for AstTestTexAdd {
    fn kind(&self) -> u16 { 65006 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestTexAdd {
    fn create_by(zhu: u8) -> Self { Self { zhu_add: Uint1::from(zhu) } }
}

// --- Test helper: action that sets P2SH with configurable address byte ---
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestP2shSetN {
    addr_byte: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestP2shSetN {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.addr_byte.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestP2shSetN {
    fn serialize(&self) -> Vec<u8> { self.addr_byte.serialize() }
    fn size(&self) -> usize { self.addr_byte.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestP2shSetN { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestP2shSetN {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"addr_byte\":{}}}", self.addr_byte.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestP2shSetN {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs { if k == "addr_byte" { self.addr_byte.from_json(v)?; } }
        Ok(())
    }
}
#[cfg(feature = "ast")]
struct AstTestP2shImpl;
#[cfg(feature = "ast")]
impl P2sh for AstTestP2shImpl {
    fn code_stuff(&self) -> &[u8] { b"code" }
    fn witness(&self) -> &[u8] { b"wit" }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestP2shSetN {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestP2shSetN {}
#[cfg(feature = "ast")]
impl Action for AstTestP2shSetN {
    fn kind(&self) -> u16 { 65007 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestP2shSetN {
    fn create_by(n: u8) -> Self { Self { addr_byte: Uint1::from(n) } }
}

// --- Test helper: action that does state set + tex + log in one shot ---
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestCombo {
    key: Uint1,
    val: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestCombo {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestCombo {
    fn serialize(&self) -> Vec<u8> { [self.key.serialize(), self.val.serialize()].concat() }
    fn size(&self) -> usize { self.key.size() + self.val.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestCombo { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestCombo {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"key\":{},\"val\":{}}}", self.key.to_json_fmt(fmt), self.val.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestCombo {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs {
            if k == "key" { self.key.from_json(v)?; }
            else if k == "val" { self.val.from_json(v)?; }
        }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestCombo {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        ctx.tex_ledger().zhu += *self.val as i64;
        ctx.logs().push(&self.key);
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestCombo {}
#[cfg(feature = "ast")]
impl Action for AstTestCombo {
    fn kind(&self) -> u16 { 65008 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestCombo {
    fn create_by(key: u8, val: u8) -> Self {
        Self { key: Uint1::from(key), val: Uint1::from(val) }
    }
}

// --- In-memory Logs for testing snapshot_len / truncate ---
#[cfg(feature = "ast")]
struct AstTestLogs {
    entries: Vec<Vec<u8>>,
}
#[cfg(feature = "ast")]
impl AstTestLogs {
    fn new() -> Self { Self { entries: vec![] } }
    fn len(&self) -> usize { self.entries.len() }
}
#[cfg(feature = "ast")]
impl Logs for AstTestLogs {
    fn push(&mut self, stuff: &dyn Serialize) {
        self.entries.push(stuff.serialize());
    }
    fn snapshot_len(&self) -> usize { self.entries.len() }
    fn truncate(&mut self, len: usize) { self.entries.truncate(len); }
}

// --- Helper to build ctx with AstTestLogs ---
#[cfg(feature = "ast")]
fn build_ast_ctx_with_logs<'a>(
    env: Env,
    sta: Box<dyn State>,
    log: Box<dyn Logs>,
    tx: &'a dyn TransactionRead,
) -> crate::context::ContextInst<'a> {
    crate::context::ContextInst::new(env, sta, log, tx)
}

// PLACEHOLDER_NEW_TESTS

// ---- Test 1: AstIf branch failure triggers whole_snap recover ----
// Validates the fix: without ctx_recover(ctx, whole_snap) on branch Err,
// the state fork layer leaks.
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_branch_fail_recovers_whole_snap() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![200], vec![200]); // baseline

    // cond succeeds (writes state), but br_if fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(201, 201))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(202, 202))]),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least") || err.contains("ast test forced fail"));

    // whole_snap must have been recovered: cond side-effects rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 200), Some(200)); // baseline intact
    assert_eq!(ast_state_get_u8(&mut ctx, 201), None); // cond write rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 202), None); // else never ran
}

// ---- Test 2: AstIf else branch failure also recovers whole_snap ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_else_branch_fail_recovers_whole_snap() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // cond fails -> else branch, but else also fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(210, 210))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("ast test forced fail") || err.contains("must succeed at least"));
    assert_eq!(ast_state_get_u8(&mut ctx, 210), None);
}

// ---- Test 3: AstSelect early-return validation doesn't leak state fork ----
// Validates the fix: validation checks moved before ctx_snapshot.
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_validation_early_return_no_state_leak() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![220], vec![220]);

    // min > max: invalid
    let bad = AstSelect::create_by(3, 1, vec![Box::new(AstTestSet::create_by(221, 221))]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = bad.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot less than min"));

    // State must still be usable (no leaked fork layer)
    assert_eq!(ast_state_get_u8(&mut ctx, 220), Some(220));
    ctx.state().set(vec![222], vec![222]);
    assert_eq!(ast_state_get_u8(&mut ctx, 222), Some(222));
}

// PLACEHOLDER_TESTS_PART2

// ---- Test 4: Logs are truncated on AstSelect child failure ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_logs_truncated_on_child_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    // child 1: log + succeed, child 2: log + fail
    // AstSelect(min=1, max=2): child1 ok, child2 fail -> ok with 1
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestLog::create_by(1)),
        Box::new(AstTestFail::new()), // fails, its snap should recover logs
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    // Only child1's log should remain (child2 failed -> no log from it, but AstTestFail doesn't log)
    // The key point: log count should be 1 (from child1), not more
    let log_len = unsafe { &*logs_ptr }.len();
    assert_eq!(log_len, 1);
}

// ---- Test 5: Logs truncated on AstIf branch failure (whole_snap recover) ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_branch_fail_truncates_logs() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

    // cond logs + succeeds, br_if logs + fails -> whole_snap recover should truncate all
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestLog::create_by(10))]),
        AstSelect::create_by(2, 2, vec![
            Box::new(AstTestLog::create_by(11)),
            Box::new(AstTestFail::new()),
        ]),
        AstSelect::create_list(vec![Box::new(AstTestLog::create_by(12))]),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // All logs from cond and br_if must be rolled back
    let log_len = unsafe { &*logs_ptr }.len();
    assert_eq!(log_len, 0);
}

// ---- Test 6: tex_ledger restored on nested AstSelect failure ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_tex_ledger_restored_on_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.tex_ledger().zhu = 100; // baseline

    // child1: adds 10 to zhu + succeeds
    // child2: adds 20 to zhu + fails
    // min=1, max=2 -> child1 ok, child2 fail -> ok
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestTexAdd::create_by(10)),
        Box::new(AstTestFail::new()),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    // child1's tex change committed, child2 never modified tex (AstTestFail doesn't touch it)
    assert_eq!(ctx.tex_ledger().zhu, 110);
}

// ---- Test 7: tex_ledger fully rolled back when AstIf fails ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_fail_rolls_back_tex_ledger() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.tex_ledger().zhu = 50;

    // cond adds 5 to zhu + succeeds, br_if fails
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestTexAdd::create_by(5))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // whole_snap recover must restore tex_ledger
    assert_eq!(ctx.tex_ledger().zhu, 50);
}

// PLACEHOLDER_TESTS_PART3

// ---- Test 8: P2SH set in successful branch kept, failed branch removed ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_p2sh_kept_on_success_removed_on_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // child1: set p2sh(addr_byte=30) + succeed
    // child2: set p2sh(addr_byte=31) + fail (wrapped in select that requires 2 but only 1 succeeds)
    let inner_fail = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestP2shSetN::create_by(31)),
        Box::new(AstTestFail::new()),
    ]);
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestP2shSetN::create_by(30)),
        Box::new(inner_fail),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    let adr30 = Address::create_scriptmh([30u8; 20]);
    let adr31 = Address::create_scriptmh([31u8; 20]);
    assert!(ctx.p2sh(&adr30).is_ok()); // success branch kept
    assert!(ctx.p2sh(&adr31).is_err()); // failed branch removed
}

// ---- Test 9: AstSelect min=0 all children fail -> success with empty result ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_min_zero_all_fail_succeeds() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![230], vec![230]);

    let act = AstSelect::create_by(0, 2, vec![
        Box::new(AstTestFail::new()),
        Box::new(AstTestFail::new()),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap(); // should succeed

    assert_eq!(ast_state_get_u8(&mut ctx, 230), Some(230)); // baseline intact
}

// ---- Test 10: Combo action (state+tex+log) all restored on failure ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_combo_all_channels_restored_on_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.tex_ledger().zhu = 10;

    // combo writes state + tex + log, then fail forces rollback
    let act = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestCombo::create_by(240, 5)),
        Box::new(AstTestFail::new()),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least"));

    assert_eq!(ast_state_get_u8(&mut ctx, 240), None); // state rolled back
    assert_eq!(ctx.tex_ledger().zhu, 10); // tex rolled back
    assert_eq!(unsafe { &*logs_ptr }.len(), 0); // logs rolled back
}

// ---- Test 11: Nested AstIf inside AstSelect — inner if fails, outer select recovers ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_nested_if_fail_inside_select_recovers_all_channels() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
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
    let act = AstSelect::create_by(2, 3, vec![
        Box::new(AstTestCombo::create_by(251, 2)),
        Box::new(inner_if),
        Box::new(AstTestCombo::create_by(252, 3)),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_state_overwrite_in_failed_branch_does_not_leak() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![1], vec![100]); // pre-existing value

    // child1: overwrite key=1 to 200, then fail
    let inner = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestSet::create_by(1, 200)),
        Box::new(AstTestFail::new()),
    ]);
    let act = AstSelect::create_by(0, 1, vec![Box::new(inner)]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    // Original value must be preserved
    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(100));
}

// ---- Test 13: AstIf else path with nested AstSelect partial success ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_else_with_nested_select_partial_success() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // cond fails -> else branch
    // else = select(min=1, max=3): child1 ok, child2 fail, child3 ok
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
        AstSelect::create_by(1, 3, vec![
            Box::new(AstTestSet::create_by(160, 160)),
            Box::new(AstTestFail::new()),
            Box::new(AstTestSet::create_by(162, 162)),
        ]),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 160), Some(160));
    assert_eq!(ast_state_get_u8(&mut ctx, 162), Some(162));
}

// ---- Test 14: P2SH + state + tex all committed on success path ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_all_channels_committed_on_success() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);

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
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 170), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 171), Some(10));
    assert_eq!(ctx.tex_ledger().zhu, 30); // combo(10) + tex_add(20)
    assert!(ctx.p2sh(&Address::create_scriptmh([40u8; 20])).is_ok());
    // logs: combo pushed 1, log pushed 1 = 2
    assert_eq!(unsafe { &*logs_ptr }.len(), 2);
}

// ---- Test 15: Double nested AstIf — inner else, outer if ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_double_nested_if_inner_else_outer_if() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    outer_if.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 182), Some(182)); // outer cond
    assert_eq!(ast_state_get_u8(&mut ctx, 181), Some(181)); // inner else
    assert_eq!(ast_state_get_u8(&mut ctx, 183), Some(183)); // outer if sibling
    assert_eq!(ast_state_get_u8(&mut ctx, 180), None); // inner if not taken
}

// ---- Test 16: AstSelect max reached stops early, remaining children not executed ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_stops_at_max() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestSet::create_by(190, 1)),
        Box::new(AstTestSet::create_by(191, 2)),
        Box::new(AstTestSet::create_by(192, 3)), // should not execute
        Box::new(AstTestSet::create_by(193, 4)), // should not execute
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 190), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 191), Some(2));
    assert_eq!(ast_state_get_u8(&mut ctx, 192), None); // not reached
    assert_eq!(ast_state_get_u8(&mut ctx, 193), None); // not reached
}

// ---- Test 17: AstSelect validation max > num rejected without state leak ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_max_gt_num_rejected_no_leak() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![1], vec![1]);

    let bad = AstSelect::create_by(1, 5, vec![
        Box::new(AstTestSet::create_by(2, 2)),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = bad.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot more than list num"));

    // State still usable
    assert_eq!(ast_state_get_u8(&mut ctx, 1), Some(1));
    ctx.state().set(vec![3], vec![3]);
    assert_eq!(ast_state_get_u8(&mut ctx, 3), Some(3));
}

// ---- Test 18: Sequential AST operations on same context ----
// After one AST op completes (success or fail), the next one works correctly.
#[cfg(feature = "ast")]
#[test]
fn test_ast_sequential_operations_on_same_context() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // Op 1: fails
    let fail_act = AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _ = fail_act.execute(&mut ctx);

    // Op 2: succeeds
    let ok_act = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(150, 150))]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    ok_act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 150), Some(150));

    // Op 3: AstIf succeeds
    let if_act = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(151, 151))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(152, 152))]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    if_act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 151), Some(151));
    assert_eq!(ast_state_get_u8(&mut ctx, 152), Some(152));
}

// ---- Test 19: P2SH duplicate address rejected even across AST branches ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_p2sh_duplicate_address_rejected() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // First set p2sh(50) outside AST
    let adr50 = Address::create_scriptmh([50u8; 20]);
    ctx.p2sh_set(adr50, Box::new(AstTestP2shImpl)).unwrap();

    // Try to set same address inside AST -> should fail
    let act = AstSelect::create_list(vec![Box::new(AstTestP2shSetN::create_by(50))]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.contains("already proved") || err.contains("must succeed at least"),
        "unexpected error: {}", err);
}

// ---- Test 20: P2SH set in failed AstSelect child is rolled back,
//               then same address can be set in next successful child ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_p2sh_rollback_allows_retry_in_next_child() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // child1: set p2sh(60) then fail -> rolled back
    // child2: set p2sh(60) succeeds (because child1's set was rolled back)
    let inner_fail = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestP2shSetN::create_by(60)),
        Box::new(AstTestFail::new()),
    ]);
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(inner_fail),
        Box::new(AstTestP2shSetN::create_by(60)),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    let adr60 = Address::create_scriptmh([60u8; 20]);
    assert!(ctx.p2sh(&adr60).is_ok());
}

// =====================================================================
// VM snapshot/restore tests within AST branches
// =====================================================================

// --- Mock VM that tracks global state for snapshot/restore testing ---
#[cfg(feature = "ast")]
struct MockVM {
    /// Mutable counter that simulates VM global state changes.
    counter: std::sync::Arc<std::sync::atomic::AtomicI64>,
}

#[cfg(feature = "ast")]
impl MockVM {
    fn create() -> (Box<dyn VM>, std::sync::Arc<std::sync::atomic::AtomicI64>) {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
        (Box::new(Self { counter: counter.clone() }), counter)
    }
}

#[cfg(feature = "ast")]
impl VM for MockVM {
    fn usable(&self) -> bool { true }

    fn snapshot_volatile(&self) -> Box<dyn Any> {
        Box::new(self.counter.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        if let Ok(c) = snap.downcast::<i64>() {
            self.counter.store(*c, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

// --- Test action that mutates VM state (increments MockVM counter) ---
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestVMCall {
    increment: Uint1,
}

#[cfg(feature = "ast")]
impl Parse for AstTestVMCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.increment.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestVMCall {
    fn serialize(&self) -> Vec<u8> { self.increment.serialize() }
    fn size(&self) -> usize { self.increment.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestVMCall { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestVMCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String { "{}".to_owned() }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestVMCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> { Ok(()) }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestVMCall {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        // The MockVM's counter is behind an Arc<AtomicI64>, so we can
        // mutate it through the shared reference obtained via ctx.vm().
        // snapshot_volatile captures the current value; restore_volatile resets it.
        // Here we just read the snapshot to get the current counter, add our increment,
        // and "commit" by not restoring. The snapshot/restore mechanism in ctx_snapshot/
        // ctx_recover will handle rollback if needed.
        //
        // We use a trick: snapshot gives us the current value, we compute new value,
        // then restore to new value. This simulates what real VM execution does
        // (modifying global_vals in place).
        let snap = ctx.vm().snapshot_volatile();
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.increment as i64;
            // Restore to the NEW value (this is how we "write" to the MockVM)
            ctx.vm().restore_volatile(Box::new(new_val));
        }
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestVMCall {}
#[cfg(feature = "ast")]
impl Action for AstTestVMCall {
    fn kind(&self) -> u16 { 65009 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestVMCall {
    fn create_by(inc: u8) -> Self { Self { increment: Uint1::from(inc) } }
}

// PLACEHOLDER_VM_TESTS
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainSet {
    key: Uint1,
    val: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestMainSet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestMainSet {
    fn serialize(&self) -> Vec<u8> { [self.key.serialize(), self.val.serialize()].concat() }
    fn size(&self) -> usize { self.key.size() + self.val.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestMainSet { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestMainSet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!(
            "{{\"key\":{},\"val\":{}}}",
            self.key.to_json_fmt(fmt),
            self.val.to_json_fmt(fmt)
        )
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestMainSet {
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
impl ActExec for AstTestMainSet {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestMainSet {}
#[cfg(feature = "ast")]
impl Action for AstTestMainSet {
    fn kind(&self) -> u16 { 65011 }
    fn level(&self) -> ActLv { ActLv::MainCall }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestMainSet {
    fn create_by(key: u8, val: u8) -> Self {
        Self {
            key: Uint1::from(key),
            val: Uint1::from(val),
        }
    }
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainP2shSetN {
    addr_byte: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestMainP2shSetN {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.addr_byte.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestMainP2shSetN {
    fn serialize(&self) -> Vec<u8> { self.addr_byte.serialize() }
    fn size(&self) -> usize { self.addr_byte.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestMainP2shSetN { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestMainP2shSetN {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"addr_byte\":{}}}", self.addr_byte.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestMainP2shSetN {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs {
            if k == "addr_byte" {
                self.addr_byte.from_json(v)?;
            }
        }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestMainP2shSetN {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestMainP2shSetN {}
#[cfg(feature = "ast")]
impl Action for AstTestMainP2shSetN {
    fn kind(&self) -> u16 { 65012 }
    fn level(&self) -> ActLv { ActLv::MainCall }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestMainP2shSetN {
    fn create_by(n: u8) -> Self { Self { addr_byte: Uint1::from(n) } }
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMainVMCall {
    increment: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestMainVMCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.increment.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestMainVMCall {
    fn serialize(&self) -> Vec<u8> { self.increment.serialize() }
    fn size(&self) -> usize { self.increment.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestMainVMCall { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestMainVMCall {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String { "{}".to_owned() }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestMainVMCall {
    fn from_json(&mut self, _json: &str) -> Ret<()> { Ok(()) }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestMainVMCall {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        let snap = ctx.vm().snapshot_volatile();
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.increment as i64;
            ctx.vm().restore_volatile(Box::new(new_val));
        }
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestMainVMCall {}
#[cfg(feature = "ast")]
impl Action for AstTestMainVMCall {
    fn kind(&self) -> u16 { 65013 }
    fn level(&self) -> ActLv { ActLv::MainCall }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestMainVMCall {
    fn create_by(inc: u8) -> Self { Self { increment: Uint1::from(inc) } }
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestRet {
    tag: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestRet {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> { self.tag.parse(buf) }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestRet {
    fn serialize(&self) -> Vec<u8> { self.tag.serialize() }
    fn size(&self) -> usize { self.tag.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestRet { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestRet {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"tag\":{}}}", self.tag.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestRet {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs {
            if k == "tag" {
                self.tag.from_json(v)?;
            }
        }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestRet {
    fn execute(&self, _ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        Ok((0, vec![*self.tag]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestRet {}
#[cfg(feature = "ast")]
impl Action for AstTestRet {
    fn kind(&self) -> u16 { 65014 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestRet {
    fn create_by(tag: u8) -> Self { Self { tag: Uint1::from(tag) } }
}

#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestMutateAllFail {
    key: Uint1,
    val: Uint1,
    addr_byte: Uint1,
    vm_add: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestMutateAllFail {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        mv += self.addr_byte.parse(&buf[mv..])?;
        mv += self.vm_add.parse(&buf[mv..])?;
        Ok(mv)
    }
}
#[cfg(feature = "ast")]
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
#[cfg(feature = "ast")]
impl Field for AstTestMutateAllFail { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
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
#[cfg(feature = "ast")]
impl FromJSON for AstTestMutateAllFail {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
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
#[cfg(feature = "ast")]
impl ActExec for AstTestMutateAllFail {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        ctx.state().set(vec![*self.key], vec![*self.val]);
        ctx.tex_ledger().zhu += *self.val as i64;
        ctx.logs().push(&self.key);
        let adr = Address::create_scriptmh([*self.addr_byte; 20]);
        ctx.p2sh_set(adr, Box::new(AstTestP2shImpl))?;
        let snap = ctx.vm().snapshot_volatile();
        if let Ok(cur) = snap.downcast::<i64>() {
            let new_val = *cur + *self.vm_add as i64;
            ctx.vm().restore_volatile(Box::new(new_val));
        }
        errf!("ast test mutate-all fail")
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestMutateAllFail {}
#[cfg(feature = "ast")]
impl Action for AstTestMutateAllFail {
    fn kind(&self) -> u16 { 65015 }
    fn level(&self) -> ActLv { ActLv::MainCall }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_vm_state_restored_on_select_child_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    // child1: vm += 5, succeed
    // child2: vm += 10, then fail -> vm should be rolled back to 5
    let inner_fail = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestVMCall::create_by(10)),
        Box::new(AstTestFail::new()),
    ]);
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestVMCall::create_by(5)),
        Box::new(inner_fail),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
}

// ---- Test 22: VM state fully rolled back when AstIf branch fails ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_vm_state_rolled_back_on_if_branch_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    // cond: vm += 3, succeed -> br_if: vm += 7, fail
    // whole_snap recover should restore vm to 0
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]),
        AstSelect::create_by(2, 2, vec![
            Box::new(AstTestVMCall::create_by(7)),
            Box::new(AstTestFail::new()),
        ]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _err = astif.execute(&mut ctx).unwrap_err();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
}

// ---- Test 23: VM state committed on successful AstIf path ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_vm_state_committed_on_success() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    // cond: vm += 2, br_if: vm += 3 -> total 5
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(2))]),
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
}

// ---- Test 24: VM + state + tex + logs + p2sh all restored together on failure ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_all_five_channels_restored_on_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);
    ctx.tex_ledger().zhu = 100;

    // All channels modified, then fail
    let astif = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(110, 10)),  // state + tex + log
            Box::new(AstTestVMCall::create_by(5)),       // vm
            Box::new(AstTestP2shSetN::create_by(70)),    // p2sh
        ]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]), // force fail
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _err = astif.execute(&mut ctx).unwrap_err();

    // All five channels must be restored
    assert_eq!(ast_state_get_u8(&mut ctx, 110), None);
    assert_eq!(ctx.tex_ledger().zhu, 100);
    assert_eq!(unsafe { &*logs_ptr }.len(), 0);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(ctx.p2sh(&Address::create_scriptmh([70u8; 20])).is_err());
}

// ---- Test 25: VM state in nested AstIf-inside-AstSelect: inner fail isolated ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_vm_nested_if_fail_isolated_by_outer_select() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    // child1: vm += 10, ok
    // child2: AstIf(cond: vm += 20, br_if: fail) -> inner fail, outer select recovers
    // child3: vm += 30, ok
    // Expected: 10 + 30 = 40 (child2's 20 rolled back)
    let inner_if = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(20))]),
        AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]),
        AstSelect::nop(),
    );
    let act = AstSelect::create_by(2, 3, vec![
        Box::new(AstTestVMCall::create_by(10)),
        Box::new(inner_if),
        Box::new(AstTestVMCall::create_by(30)),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 40);
}

// ---- Test 26: ctx_action_call (EXTACTION) nested inside AstSelect ----
// Tests that actions created via ctx_action_call within AST branches
// have their state changes properly rolled back on failure.
#[cfg(feature = "ast")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct AstTestExtCall {
    key: Uint1,
    val: Uint1,
}
#[cfg(feature = "ast")]
impl Parse for AstTestExtCall {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let mut mv = self.key.parse(buf)?;
        mv += self.val.parse(&buf[mv..])?;
        Ok(mv)
    }
}
#[cfg(feature = "ast")]
impl Serialize for AstTestExtCall {
    fn serialize(&self) -> Vec<u8> { [self.key.serialize(), self.val.serialize()].concat() }
    fn size(&self) -> usize { self.key.size() + self.val.size() }
}
#[cfg(feature = "ast")]
impl Field for AstTestExtCall { fn new() -> Self { Self::default() } }
#[cfg(feature = "ast")]
impl ToJSON for AstTestExtCall {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        format!("{{\"key\":{},\"val\":{}}}", self.key.to_json_fmt(fmt), self.val.to_json_fmt(fmt))
    }
}
#[cfg(feature = "ast")]
impl FromJSON for AstTestExtCall {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let pairs = json_split_object(json);
        for (k, v) in pairs {
            if k == "key" { self.key.from_json(v)?; }
            else if k == "val" { self.val.from_json(v)?; }
        }
        Ok(())
    }
}
#[cfg(feature = "ast")]
impl ActExec for AstTestExtCall {
    fn execute(&self, ctx: &mut dyn Context) -> Ret<(u32, Vec<u8>)> {
        // Simulate what EXTACTION does: modify state through ctx_action_call path.
        // We directly set state here since ctx_action_call ultimately calls action.execute(ctx).
        ctx.state().set(vec![*self.key], vec![*self.val]);
        // Also modify tex to test cross-channel consistency
        ctx.tex_ledger().sat += *self.val as i64;
        Ok((0, vec![]))
    }
}
#[cfg(feature = "ast")]
impl Description for AstTestExtCall {}
#[cfg(feature = "ast")]
impl Action for AstTestExtCall {
    fn kind(&self) -> u16 { 65010 }
    fn level(&self) -> ActLv { ActLv::Ast }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
#[cfg(feature = "ast")]
impl AstTestExtCall {
    fn create_by(key: u8, val: u8) -> Self {
        Self { key: Uint1::from(key), val: Uint1::from(val) }
    }
}

// ---- Test 26: EXTACTION-like state changes rolled back in failed AstSelect child ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_extcall_state_rolled_back_on_select_child_failure() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    // child1: extcall sets key=120 val=1, sat+=1, ok
    // child2: extcall sets key=121 val=2, sat+=2, then fail -> rolled back
    let inner_fail = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestExtCall::create_by(121, 2)),
        Box::new(AstTestFail::new()),
    ]);
    let act = AstSelect::create_by(1, 2, vec![
        Box::new(AstTestExtCall::create_by(120, 1)),
        Box::new(inner_fail),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 120), Some(1));
    assert_eq!(ast_state_get_u8(&mut ctx, 121), None);
    assert_eq!(ctx.tex_ledger().sat, 1); // only child1's sat
}

// ---- Test 27: Multiple sequential AST ops with VM — state accumulates correctly ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_vm_sequential_accumulation() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    // Op1: select(vm += 3) -> ok, counter = 3
    let act1 = AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(3))]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act1.execute(&mut ctx).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);

    // Op2: select(vm += 7, fail) -> fail, counter stays 3
    let act2 = AstSelect::create_by(2, 2, vec![
        Box::new(AstTestVMCall::create_by(7)),
        Box::new(AstTestFail::new()),
    ]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _ = act2.execute(&mut ctx);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);

    // Op3: if(cond: vm += 2, br_if: vm += 4) -> ok, counter = 3 + 2 + 4 = 9
    let act3 = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(2))]),
        AstSelect::create_list(vec![Box::new(AstTestVMCall::create_by(4))]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act3.execute(&mut ctx).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 9);
}

// ---- Test 28: Deep 3-level nesting with all channels ----
// AstIf -> AstSelect -> AstIf, with VM + state + tex + logs + p2sh
#[cfg(feature = "ast")]
#[test]
fn test_ast_deep_3level_all_channels() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);

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
        AstSelect::create_list(vec![
            Box::new(lvl2),
            Box::new(AstTestTexAdd::create_by(5)),
        ]),
        AstSelect::nop(),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_cond_partial_failure_with_maincall_rolls_back_and_runs_else() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 10), None); // cond write rolled back
    assert_eq!(ast_state_get_u8(&mut ctx, 11), Some(11)); // else branch committed
    assert_eq!(ast_state_get_u8(&mut ctx, 12), None); // if branch not taken
    assert!(ctx.p2sh(&Address::create_scriptmh([90u8; 20])).is_err()); // cond p2sh rolled back
    assert!(ctx.p2sh(&Address::create_scriptmh([91u8; 20])).is_ok()); // else p2sh committed
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 6); // cond vm +4 rolled back
}

// ---- Test 30: Mixed MainCall + AST nested failure is isolated by outer AstSelect ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_nested_mixed_maincall_p2sh_vm_failure_isolated() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

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
            Box::new(nested_if_fail),                      // fail, must be isolated
            Box::new(AstTestMainP2shSetN::create_by(93)), // success #2
        ],
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    outer.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 20), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 21), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([92u8; 20])).is_err());
    assert!(ctx.p2sh(&Address::create_scriptmh([93u8; 20])).is_ok());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5); // nested +7 rolled back
}

// ---- Test 31: Deep AstIf->AstSelect->AstIf with MainCall actions commits expected channels ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_deep_maincall_if_select_if_commits_expected_state() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled

    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    lvl1.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 32), Some(32));
    assert_eq!(ast_state_get_u8(&mut ctx, 31), Some(31));
    assert_eq!(ast_state_get_u8(&mut ctx, 30), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([94u8; 20])).is_ok());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5); // 3 + 2, cond-failed +1 rolled back
}

// ---- Test 32: AstSelect rejects actions len > TX_ACTIONS_MAX without leaking state context ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_num_over_tx_actions_max_rejected_no_leak() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.state().set(vec![241], vec![241]); // baseline

    let mut acts: Vec<Box<dyn Action>> = vec![];
    for i in 0..(TX_ACTIONS_MAX + 1) {
        acts.push(Box::new(AstTestSet::create_by((i % 200) as u8, 1)));
    }
    let over = AstSelect::create_by(0, 0, acts);

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = over.execute(&mut ctx).unwrap_err();
    assert!(err.contains("num cannot more than"), "{}", err);

    // state fork should not leak; context stays usable
    assert_eq!(ast_state_get_u8(&mut ctx, 241), Some(241));
    ctx.state().set(vec![242], vec![242]);
    assert_eq!(ast_state_get_u8(&mut ctx, 242), Some(242));
}

// ---- Test 33: AstSelect max=0 short-circuits and executes no child ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_max_zero_executes_no_children() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);
    ctx.vm_replace(mock_vm);

    let act = AstSelect::create_by(
        0,
        0,
        vec![
            Box::new(AstTestSet::create_by(243, 1)),
            Box::new(AstTestP2shSetN::create_by(96)),
            Box::new(AstTestVMCall::create_by(9)),
        ],
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let rv = act.execute(&mut ctx).unwrap();
    assert_eq!(rv.1, vec![]);
    assert_eq!(ast_state_get_u8(&mut ctx, 243), None);
    assert!(ctx.p2sh(&Address::create_scriptmh([96u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
}

// ---- Test 34: AstSelect returns result bytes from the last successful child ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_returns_last_success_result_bytes() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let act = AstSelect::create_by(
        1,
        3,
        vec![
            Box::new(AstTestRet::create_by(1)),
            Box::new(AstTestFail::new()),
            Box::new(AstTestRet::create_by(3)),
        ],
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let (_, rv) = act.execute(&mut ctx).unwrap();
    assert_eq!(rv, vec![3]);
}

// ---- Test 35: AstIf returns selected branch bytes and restores ctx level ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_returns_selected_branch_result_and_restores_level() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![Box::new(AstTestFail::new())]), // cond fail => else
        AstSelect::create_list(vec![Box::new(AstTestRet::create_by(7))]),
        AstSelect::create_list(vec![Box::new(AstTestRet::create_by(8))]),
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let (_, rv) = astif.execute(&mut ctx).unwrap();
    assert_eq!(rv, vec![8]);
    assert_eq!(ctx.level(), ACTION_CTX_LEVEL_TOP);
}

// ---- Test 36: AstIf branch error still restores ctx level by AstLevelGuard ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_error_restores_ctx_level() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

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
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _ = astif.execute(&mut ctx).unwrap_err();

    assert_eq!(ctx.level(), ACTION_CTX_LEVEL_TOP);
}

// ---- Test 37: Invalid cond AstSelect in AstIf falls through to else without cond side-effects ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_invalid_cond_select_runs_else_no_cond_leak() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let astif = AstIf::create_by(
        // invalid cond: min > max, should return Err and execute else branch
        AstSelect::create_by(
            2,
            1,
            vec![Box::new(AstTestMainSet::create_by(246, 246))],
        ),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(247, 247))]),
        AstSelect::create_list(vec![Box::new(AstTestMainSet::create_by(248, 248))]),
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 246), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 247), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 248), Some(248));
}

// ---- Test 38: AstSelect child that mutates all channels then fails is fully recovered ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_direct_child_mutate_all_fail_recovers_all_channels() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);
    ctx.tex_ledger().zhu = 10;

    let child_ok = AstSelect::create_list(vec![
        Box::new(AstTestCombo::create_by(249, 2)),       // state + tex + log
        Box::new(AstTestMainP2shSetN::create_by(97)),    // p2sh
        Box::new(AstTestMainVMCall::create_by(3)),       // vm
    ]);
    let child_fail = AstTestMutateAllFail::create_by(250, 5, 98, 7); // all channels mutate then Err
    let act = AstSelect::create_by(1, 2, vec![Box::new(child_ok), Box::new(child_fail)]);

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_branch_mutate_all_fail_recovers_whole_snap_all_channels() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);
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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_num_eq_tx_actions_max_allowed() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let mut acts: Vec<Box<dyn Action>> = vec![Box::new(AstTestSet::create_by(201, 1))];
    for _ in 1..TX_ACTIONS_MAX {
        acts.push(Box::new(AstTestFail::new()));
    }
    let act = AstSelect::create_by(1, 1, acts); // should stop after first success

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    act.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 201), Some(1));
}

// ---- Test 41: AstSelect error still restores ctx level by AstLevelGuard ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_error_restores_ctx_level() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let bad = AstSelect::create_by(1, 1, vec![Box::new(AstTestFail::new())]);
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let _ = bad.execute(&mut ctx).unwrap_err();

    assert_eq!(ctx.level(), ACTION_CTX_LEVEL_TOP);
}

// ---- Test 42: AstIf with cond=nop takes if-branch (cond success with 0-required select) ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_cond_nop_takes_if_branch() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let astif = AstIf::create_by(
        AstSelect::nop(), // cond succeeds (0/0)
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(202, 202))]),
        AstSelect::create_list(vec![Box::new(AstTestSet::create_by(203, 203))]),
    );
    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    astif.execute(&mut ctx).unwrap();

    assert_eq!(ast_state_get_u8(&mut ctx, 202), Some(202));
    assert_eq!(ast_state_get_u8(&mut ctx, 203), None);
}

// ---- Test 43: ast depth exactly 6 is allowed ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_tree_depth_exact_6_is_allowed() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let lvl6 = AstSelect::create_list(vec![Box::new(AstTestSet::create_by(204, 204))]);
    let lvl5 = AstSelect::create_list(vec![Box::new(lvl6)]);
    let lvl4 = AstSelect::create_list(vec![Box::new(lvl5)]);
    let lvl3 = AstSelect::create_list(vec![Box::new(lvl4)]);
    let lvl2 = AstSelect::create_list(vec![Box::new(lvl3)]);
    let lvl1 = AstSelect::create_list(vec![Box::new(lvl2)]);

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    lvl1.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 204), Some(204));
}

// ---- Test 44: AstIf cond mutate-all fail is recovered, else commits ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_cond_mutate_all_fail_recovers_and_commits_else() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);
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

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
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
#[cfg(feature = "ast")]
#[test]
fn test_ast_if_branch_validation_error_recovers_cond_all_channels() {
    let mut tx = TransactionType2::default();
    tx.actions.push(Box::new(AstSelect::nop())).unwrap();

    let mut env = Env::default();
    env.chain.fast_sync = false; // keep check_action_level enabled
    let logs = Box::new(AstTestLogs::new());
    let logs_ptr = logs.as_ref() as *const AstTestLogs;
    let (mock_vm, counter) = MockVM::create();
    let mut ctx = build_ast_ctx_with_logs(env, Box::new(AstTestState::default()), logs, &tx);
    ctx.vm_replace(mock_vm);
    ctx.tex_ledger().zhu = 40;
    counter.store(1, std::sync::atomic::Ordering::SeqCst);
    ctx.logs().push(&Uint1::from(2)); // baseline

    let astif = AstIf::create_by(
        AstSelect::create_list(vec![
            Box::new(AstTestCombo::create_by(215, 6)),
            Box::new(AstTestMainP2shSetN::create_by(107)),
            Box::new(AstTestMainVMCall::create_by(4)),
        ]),
        AstSelect::create_by(
            3,
            1,
            vec![Box::new(AstTestMainSet::create_by(216, 216))],
        ),
        AstSelect::nop(),
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    let err = astif.execute(&mut ctx).unwrap_err();
    assert!(err.contains("max cannot less than min"), "{}", err);

    assert_eq!(ast_state_get_u8(&mut ctx, 215), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 216), None);
    assert_eq!(ctx.tex_ledger().zhu, 40);
    assert_eq!(unsafe { &*logs_ptr }.len(), 1); // baseline only
    assert!(ctx.p2sh(&Address::create_scriptmh([107u8; 20])).is_err());
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// ---- Test 46: Nested invalid AstSelect is treated as failed child and isolated ----
#[cfg(feature = "ast")]
#[test]
fn test_ast_select_nested_invalid_select_isolated() {
    let tx = TransactionType2::default();
    let mut env = Env::default();
    env.chain.fast_sync = true;
    let mut ctx = build_ast_ctx_with_state(env, Box::new(AstTestState::default()), &tx);

    let bad_nested = AstSelect::create_by(
        2,
        1,
        vec![Box::new(AstTestSet::create_by(217, 217))],
    );
    let outer = AstSelect::create_by(
        1,
        2,
        vec![
            Box::new(bad_nested), // fail as one child
            Box::new(AstTestSet::create_by(218, 218)),
        ],
    );

    ctx.level_set(ACTION_CTX_LEVEL_TOP);
    outer.execute(&mut ctx).unwrap();
    assert_eq!(ast_state_get_u8(&mut ctx, 217), None);
    assert_eq!(ast_state_get_u8(&mut ctx, 218), Some(218));
}
