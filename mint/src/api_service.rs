use std::collections::VecDeque;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use basis::difficulty::*;
use basis::interface::*;
use field::*;
use protocol::block::*;
use protocol::transaction::*;
use serde_json::{json, Value};
use sys::*;

use crate::genesis::*;
use crate::oprate::*;
use crate::*;

struct MintApiService {}

pub fn service() -> Arc<dyn ApiService> {
    Arc::new(MintApiService {})
}

impl ApiService for MintApiService {
    fn name(&self) -> &'static str {
        "mint"
    }

    fn routes(&self) -> Vec<ApiRoute> {
        vec![
            ApiRoute::get("/query/supply", supply),
            ApiRoute::get("/query/miner/notice", miner_notice),
            ApiRoute::get("/query/miner/pending", miner_pending),
            ApiRoute::get("/submit/miner/success", miner_success),
            ApiRoute::get("/query/diamondminer/init", diamondminer_init),
            ApiRoute::post("/submit/diamondminer/success", diamondminer_success),
        ]
    }
}

#[allow(dead_code)]
pub struct MinerBlockStuff {
    height: BlockHeight,
    block_nonce: Uint4,
    coinbase_nonce: Hash,
    target_hash: Hash,
    coinbase_tx: TransactionCoinbase,
    block: BlockV1,
    mrklrts: Vec<Hash>,
}

static MINER_PENDING_BLOCK: LazyLock<Arc<Mutex<VecDeque<MinerBlockStuff>>>> =
    LazyLock::new(|| Arc::default());

struct MWNCount {
    count: Arc<Mutex<u64>>,
}

impl MWNCount {
    fn new(c: Arc<Mutex<u64>>) -> Self {
        *c.lock().unwrap() += 1;
        Self { count: c }
    }
}

impl Drop for MWNCount {
    fn drop(&mut self) {
        *self.count.lock().unwrap() -= 1;
    }
}

fn api_error(errmsg: &str) -> ApiResponse {
    ApiResponse::json(json!({"ret":1,"err":errmsg}).to_string())
}

fn api_ok(data: Vec<(&str, Value)>) -> ApiResponse {
    let mut out = serde_json::Map::new();
    out.insert("ret".to_owned(), json!(0));
    for (k, v) in data {
        out.insert(k.to_owned(), v);
    }
    ApiResponse::json(Value::Object(out).to_string())
}

fn q_bool(req: &ApiRequest, key: &str, dv: bool) -> bool {
    let Some(v) = req.query(key) else {
        return dv;
    };
    match v {
        "false" | "False" | "FALSE" | "none" | "None" | "NONE" | "null" | "Null" | "NULL"
        | "0" | "_" | "" => false,
        _ => true,
    }
}

fn q_string(req: &ApiRequest, key: &str, dv: &str) -> String {
    req.query(key).map_or_else(|| dv.to_owned(), |s| s.to_owned())
}

fn q_u32(req: &ApiRequest, key: &str, dv: u32) -> u32 {
    req.query(key).and_then(|v| v.parse::<u32>().ok()).unwrap_or(dv)
}

fn body_data_may_hex(req: &ApiRequest) -> Ret<Vec<u8>> {
    if !q_bool(req, "hexbody", false) {
        return Ok(req.body.clone());
    }
    hex::decode(&req.body).map_err(|_| "hex format error".to_owned())
}

fn right_00_to_ff(hx: &mut [u8]) {
    let m = hx.len();
    for i in 0..hx.len() {
        let n = m - i - 1;
        if hx[n] == 0 {
            hx[n] = 255;
        } else {
            break;
        }
    }
}

fn encode_bytes(v: Vec<u8>, is_base64: bool) -> String {
    maybe!(is_base64, v.to_base64(), v.to_hex())
}

fn read_mint_state(ctx: &ApiExecCtx) -> Arc<Box<dyn State>> {
    ctx.engine.state()
}

fn supply(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    let lastdia = state.get_latest_diamond();
    const ZHU: u64 = 1_0000_0000;
    let supply = state.get_total_count();
    let blk_rwd = cumulative_block_reward(lasthei) * ZHU;
    let burn_fee = *supply.hacd_bid_burn_zhu + *supply.diamond_insc_burn_zhu;
    let curr_ccl = blk_rwd + *supply.channel_interest_zhu - burn_fee;
    let z2m = |zhu| zhu as f64 / ZHU as f64;
    api_ok(vec![
        ("latest_height", json!(lasthei)),
        ("current_circulation", json!(z2m(curr_ccl))),
        ("burned_fee", json!(z2m(burn_fee))),
        ("burned_diamond_bid", json!(z2m(*supply.hacd_bid_burn_zhu))),
        ("channel_deposit", json!(z2m(*supply.channel_deposit_zhu))),
        ("channel_interest", json!(z2m(*supply.channel_interest_zhu))),
        ("channel_opening", json!(*supply.opening_channel)),
        ("diamond_engraved", json!(*supply.diamond_engraved)),
        ("transferred_bitcoin", json!(0)),
        ("trsbtc_subsidy", json!(0)),
        ("block_reward", json!(z2m(blk_rwd))),
        ("minted_diamond", json!(*lastdia.number)),
    ])
}

fn update_miner_pending_block(block: BlockV1, cbtx: TransactionCoinbase) {
    let mkrluphxs = calculate_mrkl_coinbase_modify(&block.transaction_hash_list(true));
    let mut stfs = MINER_PENDING_BLOCK.lock().unwrap();
    stfs.push_front(MinerBlockStuff {
        height: block.height().clone(),
        block_nonce: Uint4::default(),
        coinbase_nonce: Hash::default(),
        target_hash: Hash::from(u32_to_hash(block.difficulty().uint())),
        coinbase_tx: cbtx,
        block,
        mrklrts: mkrluphxs,
    });
    if stfs.len() > 3 {
        stfs.pop_back();
    }
}

fn miner_reset_next_new_block(engine: Arc<dyn Engine>, txpool: &dyn TxPool) {
    let block = engine.minter().packing_next_block(engine.as_read(), txpool);
    let block = *block.downcast::<BlockV1>().unwrap();
    let cbtx: Box<dyn Transaction> = block.transactions()[0].clone();
    let cbtx: TransactionCoinbase = maybe!(
        cbtx.ty() == 0,
        TransactionCoinbase::must(&cbtx.serialize()),
        never!()
    );
    update_miner_pending_block(block, cbtx);
}

fn get_miner_pending_block_stuff(
    is_detail: bool,
    is_transaction: bool,
    is_stuff: bool,
    is_base64: bool,
) -> ApiResponse {
    let mut stuff = MINER_PENDING_BLOCK.lock().unwrap();
    if stuff.is_empty() {
        return api_error("pending block not yet");
    }
    let stuff = &mut stuff[0];

    stuff.coinbase_nonce.increase();
    stuff.coinbase_tx.set_nonce(stuff.coinbase_nonce);
    let cbhx = stuff.coinbase_tx.hash();
    let mkrl = calculate_mrkl_coinbase_update(cbhx, &stuff.mrklrts);
    stuff.block.set_mrklroot(mkrl);
    let intro_data = stuff.block.intro.serialize().to_hex();

    let mut tg_hash = stuff.target_hash.to_vec();
    right_00_to_ff(&mut tg_hash);

    let mut data = serde_json::Map::new();
    data.insert("height".to_owned(), json!(*stuff.height));
    data.insert(
        "coinbase_nonce".to_owned(),
        json!(encode_bytes(stuff.coinbase_nonce.to_vec(), is_base64)),
    );
    data.insert("block_intro".to_owned(), json!(intro_data));
    data.insert("target_hash".to_owned(), json!(encode_bytes(tg_hash, is_base64)));

    if is_detail {
        data.insert("version".to_owned(), json!(stuff.block.version().uint()));
        data.insert(
            "prevhash".to_owned(),
            json!(encode_bytes(stuff.block.prevhash().to_vec(), is_base64)),
        );
        data.insert("timestamp".to_owned(), json!(stuff.block.timestamp().uint()));
        data.insert(
            "transaction_count".to_owned(),
            json!(stuff.block.transaction_count().uint().saturating_sub(1)),
        );
        data.insert(
            "reward_address".to_owned(),
            json!(stuff.coinbase_tx.main().to_readable()),
        );
    }

    if is_transaction {
        let tx_raws: Vec<String> = stuff
            .block
            .transactions()
            .iter()
            .map(|tx| encode_bytes(tx.serialize(), is_base64))
            .collect();
        data.insert("transaction_body_list".to_owned(), json!(tx_raws));
    }

    if is_stuff {
        data.insert(
            "coinbase_body".to_owned(),
            json!(encode_bytes(stuff.coinbase_tx.serialize(), is_base64)),
        );
        let mhxs: Vec<String> = calculate_mrkl_coinbase_modify(&stuff.block.transaction_hash_list(true))
            .into_iter()
            .map(|hx| encode_bytes(hx.serialize(), is_base64))
            .collect();
        data.insert("mkrl_modify_list".to_owned(), json!(mhxs));
    }

    let mut out = serde_json::Map::new();
    out.insert("ret".to_owned(), json!(0));
    for (k, v) in data {
        out.insert(k, v);
    }
    ApiResponse::json(Value::Object(out).to_string())
}

fn miner_notice(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let target_height = req.query_u64("height", 0);
    let mut wait = req.query_u64("wait", 45);
    set_in_range!(wait, 1, 300);
    let _mwnc = MWNCount::new(ctx.miner_worker_notice_count.clone());
    let mut lasthei;
    for _ in 0..wait {
        lasthei = ctx.engine.latest_block().height().uint();
        if lasthei >= target_height {
            break;
        }
        sleep(Duration::from_secs(1));
    }
    lasthei = ctx.engine.latest_block().height().uint();
    api_ok(vec![("height", json!(lasthei))])
}

fn miner_pending(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let detail = q_bool(&req, "detail", false);
    let transaction = q_bool(&req, "transaction", false);
    let stuff = q_bool(&req, "stuff", false);
    let base64 = q_bool(&req, "base64", false);

    if !ctx.engine.config().miner_enable {
        return api_error("miner not enable");
    }

    #[cfg(not(debug_assertions))]
    {
        let gotdmintx = ctx.hnoder.txpool().first_at(TXGID_DIAMINT).unwrap().is_some();
        if ctx.engine.config().is_mainnet() && !gotdmintx && curtimes() < ctx.launch_time + 30 {
            return api_error("miner worker need launch after 30 secs for node start");
        }
    }

    let lasthei = ctx.engine.latest_block().height().uint();
    let need_create_new = {
        let stf = MINER_PENDING_BLOCK.lock().unwrap();
        if stf.is_empty() {
            true
        } else {
            *stf[0].height <= lasthei
        }
    };

    if need_create_new {
        miner_reset_next_new_block(ctx.engine.clone(), ctx.hnoder.txpool().as_ref());
    }

    get_miner_pending_block_stuff(detail, transaction, stuff, base64)
}

fn hash_diff(dst: &Hash, tar: &Hash) -> i8 {
    for i in 0..Hash::SIZE {
        if dst[i] > tar[i] {
            return 1;
        } else if dst[i] < tar[i] {
            return -1;
        }
    }
    0
}

fn miner_success(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    if !ctx.engine.config().miner_enable {
        return api_error("miner not enable");
    }

    let height = req.query_u64("height", 0);
    let block_nonce = q_u32(&req, "block_nonce", 0);
    let coinbase_nonce = q_string(&req, "coinbase_nonce", "");

    let mut success_stuff = {
        let mut stf = MINER_PENDING_BLOCK.lock().unwrap();
        if stf.is_empty() {
            return api_error("pending block not yet");
        }
        let mut found_idx = None;
        for i in 0..stf.len() {
            if *stf[i].height == height {
                found_idx = Some(i);
                break;
            }
        }
        let Some(stfidx) = found_idx else {
            return api_error(&format!("pending block height {} not find", height));
        };
        let tarstf = &mut stf[stfidx];

        let Ok(cb_nonce_bytes) = hex::decode(coinbase_nonce.as_bytes()) else {
            return api_error("coinbase nonce format error");
        };
        if cb_nonce_bytes.len() != Hash::SIZE {
            return api_error("coinbase nonce length error");
        }

        tarstf.block.set_nonce(Uint4::from(block_nonce));
        tarstf
            .coinbase_tx
            .set_nonce(Hash::from(cb_nonce_bytes.try_into().unwrap()));
        let cbhx = tarstf.coinbase_tx.hash();
        let mkrl = calculate_mrkl_coinbase_update(cbhx, &tarstf.mrklrts);
        tarstf.block.set_mrklroot(mkrl);
        let blkhx = tarstf.block.hash();
        if 1 == hash_diff(&blkhx, &tarstf.target_hash) {
            return api_error(&format!(
                "difficulty check fail: at least need {} but got {}",
                tarstf.target_hash.to_hex(),
                blkhx.to_hex()
            ));
        }
        let picked = stf.drain(stfidx..stfidx + 1).next_back().unwrap();
        picked
    };

    let done_height = success_stuff.block.height().uint();
    success_stuff
        .block
        .replace_transaction(0, Box::new(success_stuff.coinbase_tx.clone()))
        .unwrap();
    let blkpkg = BlkPkg::create(Box::new(success_stuff.block));
    if let Err(e) = ctx.hnoder.submit_block(&blkpkg, true) {
        return api_error(&format!("submit block error: {}", e));
    }
    api_ok(vec![
        ("height", json!(done_height)),
        ("mining", json!("success")),
    ])
}

fn diamondminer_init(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let cnf = ctx.engine.config();
    if !cnf.dmer_enable {
        return api_error("diamond miner in config not enable");
    }
    api_ok(vec![
        ("bid_address", json!(cnf.dmer_bid_account.readable())),
        ("reward_address", json!(cnf.dmer_reward_address.to_readable())),
    ])
}

fn diamondminer_success(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let cnf = ctx.engine.config();
    if !cnf.dmer_enable {
        return api_error("diamond miner in config not enable");
    }
    let Ok(actdts) = body_data_may_hex(&req) else {
        return api_error("hex format error");
    };
    let Ok((mint, _)) = action::DiamondMint::create(&actdts) else {
        return api_error("upload action error");
    };

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());

    let act = &mint.d;
    let mint_number = *act.number;
    let mint_name = act.diamond.to_readable();
    let lastdia = state.get_latest_diamond();
    if mint_number != *lastdia.number + 1 {
        return api_error("diamond number error");
    }
    if mint_number > 1 && act.prev_hash != lastdia.born_hash {
        return api_error("diamond prev hash error");
    }

    let bid_addr = Address::from(cnf.dmer_bid_account.address().clone());
    let mut bid_offer = cnf.dmer_bid_min.clone();
    if let Ok(Some(fbtx)) = ctx.hnoder.txpool().first_at(TXGID_DIAMINT) {
        let hbfe = fbtx.objc.fee().clone();
        let mmax = cnf.dmer_bid_max.clone();
        let step = cnf.dmer_bid_step.clone();
        if hbfe > mmax {
            bid_offer = mmax;
        } else if hbfe > bid_offer {
            if fbtx.objc.main() == bid_addr {
                bid_offer = hbfe;
            } else if let Ok(new_bid) = hbfe.add_mode_u64(&step) {
                bid_offer = new_bid;
            }
        }
    }
    if let Ok(new_bid) = bid_offer.compress(2, AmtCpr::Grow) {
        bid_offer = new_bid;
    }

    let mut tx = TransactionType2::new_by(bid_addr, bid_offer, curtimes());
    tx.push_action(Box::new(mint)).unwrap();
    tx.fill_sign(&cnf.dmer_bid_account).unwrap();
    let txhx = tx.hash();
    let txpkg = TxPkg::create(Box::new(tx));
    if let Err(e) = ctx.hnoder.submit_transaction(&txpkg, true, false) {
        return api_error(&e);
    }
    let hxstr = txhx.to_hex();
    println!(
        "▒▒▒▒ DIAMOND SUCCESS: {}({}), tx hash: {}.",
        mint_name, mint_number, hxstr
    );
    api_ok(vec![("tx_hash", json!(hxstr))])
}
