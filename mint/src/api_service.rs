use std::collections::VecDeque;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use basis::difficulty::*;
use basis::interface::*;
use field::*;
use protocol::block::*;
use protocol::state::*;
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
            ApiRoute::get("/", console),
            ApiRoute::get("/query/latest", latest),
            ApiRoute::get("/query/hashrate", hashrate),
            ApiRoute::get("/query/hashrate/logs", hashrate_logs),
            ApiRoute::get("/query/balance", balance),
            ApiRoute::get("/query/channel", channel),
            ApiRoute::get("/query/diamond", diamond),
            ApiRoute::get("/query/diamond/bidding", diamond_bidding),
            ApiRoute::get("/query/diamond/views", diamond_views),
            ApiRoute::get("/query/diamond/engrave", diamond_engrave),
            ApiRoute::get(
                "/query/diamond/inscription_protocol_cost",
                diamond_inscription_protocol_cost,
            ),
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
        "false" | "False" | "FALSE" | "none" | "None" | "NONE" | "null" | "Null" | "NULL" | "0"
        | "_" | "" => false,
        _ => true,
    }
}

fn q_string(req: &ApiRequest, key: &str, dv: &str) -> String {
    req.query(key)
        .map_or_else(|| dv.to_owned(), |s| s.to_owned())
}

fn q_u32(req: &ApiRequest, key: &str, dv: u32) -> u32 {
    req.query(key)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(dv)
}

fn q_i64(req: &ApiRequest, key: &str, dv: i64) -> i64 {
    req.query(key)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(dv)
}

fn q_f64(req: &ApiRequest, key: &str, dv: f64) -> f64 {
    req.query(key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(dv)
}

fn q_coinkind_hsd(req: &ApiRequest) -> Ret<(bool, bool, bool)> {
    let raw = q_string(req, "coinkind", "hsd");
    let mut s = raw.to_lowercase();
    s.retain(|c| !c.is_whitespace() && c != ',' && c != ';' && c != '|');
    if s.is_empty() || s == "all" || s == "hsda" {
        return Ok((true, true, true));
    }
    if !s
        .chars()
        .all(|c| c == 'h' || c == 's' || c == 'd' || c == 'a')
    {
        return errf!("coinkind format error");
    }
    Ok((s.contains('h'), s.contains('s'), s.contains('d')))
}

fn api_html(s: String) -> ApiResponse {
    ApiResponse {
        status: 200,
        headers: vec![(
            "content-type".to_owned(),
            "text/html; charset=utf-8".to_owned(),
        )],
        body: s.into_bytes(),
    }
}

fn api_data(data: serde_json::Map<String, Value>) -> ApiResponse {
    let mut out = serde_json::Map::new();
    out.insert("ret".to_owned(), json!(0));
    for (k, v) in data {
        out.insert(k, v);
    }
    ApiResponse::json(Value::Object(out).to_string())
}

fn api_data_list(list: Vec<Value>) -> ApiResponse {
    ApiResponse::json(json!({"ret":0,"list":list}).to_string())
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
    data.insert(
        "target_hash".to_owned(),
        json!(encode_bytes(tg_hash, is_base64)),
    );

    if is_detail {
        data.insert("version".to_owned(), json!(stuff.block.version().uint()));
        data.insert(
            "prevhash".to_owned(),
            json!(encode_bytes(stuff.block.prevhash().to_vec(), is_base64)),
        );
        data.insert(
            "timestamp".to_owned(),
            json!(stuff.block.timestamp().uint()),
        );
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
        let mhxs: Vec<String> =
            calculate_mrkl_coinbase_modify(&stuff.block.transaction_hash_list(true))
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
        let gotdmintx = ctx
            .hnoder
            .txpool()
            .first_at(TXGID_DIAMINT)
            .unwrap()
            .is_some();
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
        (
            "reward_address",
            json!(cnf.dmer_reward_address.to_readable()),
        ),
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

fn console(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let store = ctx.engine.store();
    let mtcnf = ctx.engine.minter().config().downcast::<MintConf>().unwrap();

    let latest = ctx.engine.latest_block();
    let lathei = latest.height().uint() as i64;
    let latts = latest.timestamp().uint();

    let cyln = mtcnf.difficulty_adjust_blocks as i64;
    let secnp = ["day", "week", "month", "quarter", "year", "all"];
    let secn = [cyln, cyln * 7, cyln * 30, cyln * 90, cyln * 365, lathei - 1];
    let mut target_time = Vec::with_capacity(secn.len());

    for i in 0..secn.len() {
        let sb = secn[i];
        let hei = lathei - sb;
        if hei <= 0 {
            break;
        }
        let Some((_, blkdts)) = store.block_data_by_height(&BlockHeight::from(hei as u64)) else {
            break;
        };
        let mut bhd = BlockIntro::default();
        if bhd.parse(&blkdts).is_err() {
            break;
        }
        let blkt = bhd.timestamp().uint();
        target_time.push(format!("{}: {}s", secnp[i], (latts - blkt) / (sb as u64)));
    }

    let poworkers = {
        let n = ctx.miner_worker_notice_count.lock().unwrap();
        *n
    };

    api_html(format!(
        r#"<html><head><title>Hacash node console</title></head><body>
        <h3>Hacash console</h3>
        <p>Latest height {} time {}</p>
        <p>Block span times: {}</p>
        <p>P2P peers: {}</p>
        <p>{}</p>
        <p>Miner worker notice connected: {}</p>
    </body></html>"#,
        latest.height().uint(),
        timeshow(latest.timestamp().uint()),
        target_time.join(", "),
        ctx.hnoder.all_peer_prints().join(", "),
        ctx.hnoder.txpool().print(),
        poworkers,
    ))
}

fn load_block_by_height(ctx: &ApiExecCtx, height: u64) -> Ret<Arc<BlkPkg>> {
    let store = ctx.engine.store();
    let Some((_, blkdts)) = store.block_data_by_height(&BlockHeight::from(height)) else {
        return errf!("block not find");
    };
    let Ok(blkpkg) = build_block_package(blkdts) else {
        return errf!("block parse error");
    };
    Ok(Arc::new(blkpkg))
}

fn query_hashrate(ctx: &ApiExecCtx) -> serde_json::Map<String, Value> {
    let mtcnf = ctx.engine.minter().config().downcast::<MintConf>().unwrap();
    let btt = mtcnf.each_block_target_time as f64;
    let lastblk = ctx.engine.latest_block();
    let curhei = lastblk.height().uint();
    let tg_difn = lastblk.difficulty().uint();
    let mut tg_hash = u32_to_hash(tg_difn);
    let tg_rate = hash_to_rates(&tg_hash, btt);
    let tg_show = rates_to_show(tg_rate);

    let mut rt_rate = tg_rate;
    let mut rt_show = tg_show.clone();
    let ltc = 100u64;
    if curhei > ltc {
        if let Ok(pblk) = load_block_by_height(ctx, curhei - ltc) {
            let p100t = pblk.objc.timestamp().uint();
            let cttt = (lastblk.timestamp().uint() - p100t) / ltc;
            if cttt > 0 {
                rt_rate = rt_rate * btt / cttt as f64;
                rt_show = rates_to_show(rt_rate);
            }
        }
    }

    right_00_to_ff(&mut tg_hash);
    let mut data = serde_json::Map::new();
    data.insert(
        "target".to_owned(),
        json!({
            "rate": tg_rate,
            "show": tg_show,
            "hash": hex::encode(&tg_hash),
            "difn": tg_difn,
        }),
    );
    data.insert(
        "realtime".to_owned(),
        json!({
            "rate": rt_rate,
            "show": rt_show,
        }),
    );
    data
}

fn hashrate(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    api_data(query_hashrate(ctx))
}

fn get_blk_rate(ctx: &ApiExecCtx, hei: u64) -> Ret<u128> {
    let difn = load_block_by_height(ctx, hei)?.objc.difficulty().uint();
    let mtcnf = ctx.engine.minter().config().downcast::<MintConf>().unwrap();
    let tms = mtcnf.each_block_target_time as f64 * 1000.0;
    Ok(u32_to_rates(difn, tms) as u128)
}

fn hashrate_logs(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let mut days = req.query_u64("days", 200);
    if days == 0 {
        days = 1;
    }
    let target = q_bool(&req, "target", false);
    let scale = q_f64(&req, "scale", 0.0);

    let mtcnf = ctx.engine.minter().config().downcast::<MintConf>().unwrap();
    let bac = mtcnf.difficulty_adjust_blocks;

    if days > 500 {
        return api_error("param days cannot more than 500");
    }
    let lasthei = ctx.engine.latest_block().height().uint();
    if lasthei < days {
        return api_error("param days overflow");
    }
    let secs = lasthei / days;

    let mut day200 = Vec::with_capacity(days as usize);
    let mut dayall = Vec::with_capacity(days as usize);
    let mut day200_max = 0u128;
    let mut dayall_max = 0u128;
    for i in 0..days {
        let s1 = lasthei - ((days - 1 - i) * bac);
        let s2 = secs + secs * i;
        let rt1 = get_blk_rate(ctx, s1).unwrap_or(0);
        let rt2 = get_blk_rate(ctx, s2).unwrap_or(0);
        if rt1 > day200_max {
            day200_max = rt1;
        }
        if rt2 > dayall_max {
            dayall_max = rt2;
        }
        day200.push(rt1);
        dayall.push(rt2);
    }

    if scale > 0.0 {
        if day200_max > 0 {
            let sd2 = day200_max as f64 / scale;
            for it in day200.iter_mut() {
                *it = (*it as f64 / sd2) as u128;
            }
        }
        if dayall_max > 0 {
            let sda = dayall_max as f64 / scale;
            for it in dayall.iter_mut() {
                *it = (*it as f64 / sda) as u128;
            }
        }
    }

    let mut data = serde_json::Map::new();
    if target {
        data = query_hashrate(ctx);
    }
    data.insert("day200".to_owned(), json!(day200));
    data.insert("dayall".to_owned(), json!(dayall));
    api_data(data)
}

fn latest(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    let lastdia = state.get_latest_diamond();
    api_ok(vec![
        ("height", json!(lasthei)),
        ("diamond", json!(*lastdia.number)),
    ])
}

fn balance(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let diamonds = q_bool(&req, "diamonds", false);
    let assets = q_bool(&req, "assets", false);
    let asset = req.query("asset").map(|s| s.to_owned());
    let (show_hacash, show_satoshi, show_diamond) = match q_coinkind_hsd(&req) {
        Ok(v) => v,
        Err(e) => return api_error(&e),
    };
    let ads = q_string(&req, "address", "")
        .replace(' ', "")
        .replace('\n', "");
    let addrs: Vec<_> = ads.split(',').collect();
    let adrsz = addrs.len();
    if adrsz == 0 || (adrsz == 1 && addrs[0].is_empty()) {
        return api_error("address format error");
    }
    if adrsz > 200 {
        return api_error("address max 200");
    }

    let staptr = read_mint_state(ctx);
    let core = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let mstate = MintStateRead::wrap(staptr.as_ref().as_ref());
    let mut resbls = Vec::with_capacity(adrsz);
    for a in addrs {
        let Ok(adr) = Address::from_readable(a) else {
            return api_error(&format!("address {} format error", a));
        };
        let bls = core.balance(&adr).unwrap_or_default();
        let mut one = serde_json::Map::new();
        if show_hacash {
            one.insert("hacash".to_owned(), json!(bls.hacash.to_unit_string(&unit)));
        }
        if show_diamond {
            one.insert("diamond".to_owned(), json!(*bls.diamond));
        }
        if show_satoshi {
            one.insert("satoshi".to_owned(), json!(*bls.satoshi));
        }
        let mut resj = Value::Object(one);

        if diamonds && show_diamond {
            let diaowned = mstate.diamond_owned(&adr).unwrap_or_default();
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("diamonds".to_owned(), json!(diaowned.readable()));
            }
        }

        if let Some(ast) = asset.as_ref() {
            let mut astlist = Vec::new();
            let mut astptr = &astlist;
            match ast.parse::<u64>() {
                Ok(astn) => {
                    if let Some(astobj) = bls.asset(Fold64::from(astn).unwrap_or(Fold64::max())) {
                        astlist.push(astobj);
                        astptr = &astlist;
                    }
                }
                _ => astptr = bls.assets.list(),
            };
            let mut arr = vec![];
            for it in astptr {
                arr.push(json!({
                    "serial": *it.serial,
                    "amount": *it.amount,
                }));
            }
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("assets".to_owned(), json!(arr));
            }
        }

        if assets {
            let mut arr = vec![];
            for it in bls.assets.list() {
                arr.push(json!({
                    "serial": *it.serial,
                    "amount": *it.amount,
                }));
            }
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("assets".to_owned(), json!(arr));
            }
        }

        resbls.push(resj);
    }
    api_data_list(resbls)
}

fn channel(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let id = q_string(&req, "id", "");
    let Ok(id) = hex::decode(&id) else {
        return api_error("channel id format error");
    };
    if id.len() != ChannelId::SIZE {
        return api_error("channel id format error");
    }
    let chid = ChannelId::must(&id);

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let Some(channel) = state.channel(&chid) else {
        return api_error("channel not find");
    };

    let status = *channel.status;
    let mut data = serde_json::Map::new();
    data.insert("id".to_owned(), json!(chid.to_hex()));
    data.insert("status".to_owned(), json!(status));
    data.insert("open_height".to_owned(), json!(*channel.open_height));
    data.insert("reuse_version".to_owned(), json!(*channel.reuse_version));
    data.insert(
        "arbitration_lock".to_owned(),
        json!(*channel.arbitration_lock_block),
    );
    data.insert(
        "interest_attribution".to_owned(),
        json!(*channel.interest_attribution),
    );
    data.insert(
        "left".to_owned(),
        json!({
            "address": channel.left_bill.address.to_readable(),
            "hacash": channel.left_bill.balance.hacash.to_unit_string(&unit),
            "satoshi": channel.left_bill.balance.satoshi.uint(),
        }),
    );
    data.insert(
        "right".to_owned(),
        json!({
            "address": channel.right_bill.address.to_readable(),
            "hacash": channel.right_bill.balance.hacash.to_unit_string(&unit),
            "satoshi": channel.right_bill.balance.satoshi.uint(),
        }),
    );

    if let Some(challenging) = channel.if_challenging.if_value() {
        let l_or_r = challenging.assert_address_is_left_or_right.check();
        let assaddr = maybe!(
            l_or_r,
            channel.left_bill.address.to_readable(),
            channel.right_bill.address.to_readable()
        );
        data.insert(
            "challenging".to_owned(),
            json!({
                "launch_height": *challenging.challenge_launch_height,
                "assert_bill_auto_number": *challenging.assert_bill_auto_number,
                "assert_address_is_left_or_right": l_or_r,
                "assert_bill": {
                    "address": assaddr,
                    "hacash": challenging.assert_bill.amount.to_unit_string(&unit),
                    "satoshi": challenging.assert_bill.satoshi.value().uint(),
                },
            }),
        );
    }

    if let Some(distribution) = channel.if_distribution.if_value() {
        data.insert(
            "distribution".to_owned(),
            json!({
                "hacash": distribution.left_bill.hacash.to_unit_string(&unit),
                "satoshi": distribution.left_bill.satoshi.uint(),
            }),
        );
    }

    api_data(data)
}

fn diamond(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut name = q_string(&req, "name", "");
    let number = req.query_u64("number", 0) as u32;

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    if number > 0 {
        let Some(dian) = state.diamond_name(&DiamondNumber::from(number)) else {
            return api_error("cannot find diamond");
        };
        name = dian.to_readable();
    } else if !DiamondName::is_valid(name.as_bytes()) {
        return api_error("diamond name error");
    }

    let dian = DiamondName::from(name.as_bytes().try_into().unwrap());
    let Some(diaobj) = state.diamond(&dian) else {
        return api_error("cannot find diamond");
    };
    let Some(diasmelt) = state.diamond_smelt(&dian) else {
        return api_error("cannot find diamond");
    };

    let mut data = serde_json::Map::new();
    data.insert("name".to_owned(), json!(dian.to_readable()));
    data.insert("belong".to_owned(), json!(diaobj.address.to_readable()));
    data.insert("inscriptions".to_owned(), json!(diaobj.inscripts.array()));
    data.insert("number".to_owned(), json!(*diasmelt.number));
    data.insert(
        "miner".to_owned(),
        json!(diasmelt.miner_address.to_readable()),
    );
    data.insert(
        "born".to_owned(),
        json!({
            "height": *diasmelt.born_height,
            "hash": diasmelt.born_hash.to_hex(),
        }),
    );
    data.insert("prev_hash".to_owned(), json!(diasmelt.prev_hash.to_hex()));
    data.insert(
        "bid_fee".to_owned(),
        json!(diasmelt.bid_fee.to_unit_string(&unit)),
    );
    data.insert(
        "average_bid_burn".to_owned(),
        json!(*diasmelt.average_bid_burn),
    );
    data.insert("life_gene".to_owned(), json!(diasmelt.life_gene.to_hex()));
    data.insert(
        "visual_gene".to_owned(),
        json!(calculate_diamond_visual_gene(&dian, &diasmelt.life_gene).to_hex()),
    );
    api_data(data)
}

fn diamond_bidding(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let limit = req.query_usize("limit", 20);
    let number = req.query_usize("number", 0) as u32;
    let since = q_bool(&req, "since", false);

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let lastdia = state.get_latest_diamond();
    let txpool = ctx.hnoder.txpool();
    let mut datalist = vec![];

    let mut pick_dmint = |a: &TxPkg| {
        if datalist.len() >= limit {
            return false;
        }
        let txhx = a.hash;
        let txr = a.objc.as_ref().as_read();
        let Some(diamtact) = action::pickout_diamond_mint_action(txr) else {
            return true;
        };
        let act = diamtact.d;
        if number > 0 && number != *act.number {
            return true;
        }
        let mut one = json!({
            "tx": txhx.to_hex(),
            "fee": txr.fee().to_unit_string(&unit),
            "bid": txr.main().to_readable(),
            "name": act.diamond.to_readable(),
            "belong": act.address.to_readable(),
        });
        if number == 0 {
            one.as_object_mut()
                .unwrap()
                .insert("number".to_owned(), json!(*act.number));
        }
        datalist.push(one);
        true
    };
    txpool.iter_at(TXGID_DIAMINT, &mut pick_dmint).unwrap();

    let mut data = serde_json::Map::new();
    data.insert("number".to_owned(), json!(*lastdia.number + 1));
    data.insert("list".to_owned(), json!(datalist));

    if since {
        if let Ok(blk) = load_block_by_height(ctx, lastdia.born_height.uint()) {
            data.insert("since".to_owned(), json!(blk.objc.timestamp().uint()));
        }
    }
    api_data(data)
}

fn get_id_range(max: i64, page: i64, limit: i64, instart: i64, desc: bool) -> Vec<i64> {
    let mut start = 1;
    if instart != i64::MAX {
        start = instart;
    }
    if desc && instart == i64::MAX {
        start = max;
    }
    if page > 1 {
        if desc {
            start -= (page - 1) * limit;
        } else {
            start += (page - 1) * limit;
        }
    }
    let mut end = start + limit;
    if desc {
        end = start - limit;
    }
    let mut rng: Vec<_> = (start..end).collect();
    if desc {
        rng = (end + 1..start + 1).rev().collect();
    }
    rng.retain(|&x| x >= 1 || x <= max);
    rng
}

fn diamond_views(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut limit = q_i64(&req, "limit", 20);
    let page = q_i64(&req, "page", 1);
    let start = q_i64(&req, "start", i64::MAX);
    let desc = q_bool(&req, "desc", false);
    let name = q_string(&req, "name", "");

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let lastdianum = *state.get_latest_diamond().number as i64;
    if limit > 200 {
        limit = 200;
    }

    let mut datalist = vec![];
    let mut query_item = |dian: &DiamondName| {
        let Some(..) = state.diamond(dian) else {
            return;
        };
        let Some(diasmelt) = state.diamond_smelt(dian) else {
            return;
        };
        datalist.push(json!({
            "name": dian.to_readable(),
            "number": *diasmelt.number,
            "bid_fee": diasmelt.bid_fee.to_unit_string(&unit),
            "life_gene": diasmelt.life_gene.to_hex(),
        }));
    };

    if name.len() >= DiamondName::SIZE {
        let Ok(names) = DiamondNameListMax200::from_readable(&name) else {
            return api_error("diamond name error");
        };
        for dian in names.list() {
            query_item(&dian);
        }
    } else {
        for id in get_id_range(lastdianum, page, limit, start, desc) {
            let Some(dian) = state.diamond_name(&DiamondNumber::from(id as u32)) else {
                continue;
            };
            query_item(&dian);
        }
    }

    let mut data = serde_json::Map::new();
    data.insert("latest_number".to_owned(), json!(lastdianum));
    data.insert("list".to_owned(), json!(datalist));
    api_data(data)
}

fn diamond_engrave(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let height = req.query_u64("height", 0);
    let tx_hash = q_bool(&req, "tx_hash", false);
    let txposi = q_i64(&req, "txposi", -1);

    let blkpkg = match load_block_by_height(ctx, height) {
        Ok(v) => v,
        Err(e) => return api_error(&e),
    };
    let trs = blkpkg.objc.transactions();
    if trs.is_empty() {
        return api_error("transaction len error");
    }
    if txposi >= 0 && txposi >= trs.len() as i64 - 1 {
        return api_error("txposi overflow");
    }

    let mut datalist = vec![];
    let mut pick_engrave = |tx: &dyn TransactionRead| {
        let txhx = tx.hash();
        for act in tx.actions() {
            if let Some(a) = action::DiamondInscription::downcast(act) {
                let mut obj = serde_json::Map::new();
                obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
                obj.insert(
                    "inscription".to_owned(),
                    json!(a.engraved_content.to_readable_or_hex()),
                );
                if tx_hash {
                    obj.insert("tx_hash".to_owned(), json!(txhx.to_hex()));
                }
                datalist.push(Value::Object(obj));
            } else if let Some(a) = action::DiamondInscriptionClear::downcast(act) {
                let mut obj = serde_json::Map::new();
                obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
                obj.insert("clear".to_owned(), json!(true));
                if tx_hash {
                    obj.insert("tx_hash".to_owned(), json!(txhx.to_hex()));
                }
                datalist.push(Value::Object(obj));
            }
        }
    };

    if txposi >= 0 {
        let tx = trs[txposi as usize + 1].as_read();
        pick_engrave(tx);
    } else {
        for tx in &trs[1..] {
            pick_engrave(tx.as_read());
        }
    }

    let mut data = serde_json::Map::new();
    data.insert("list".to_owned(), json!(datalist));
    api_data(data)
}

fn diamond_inscription_protocol_cost(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let name = q_string(&req, "name", "");
    let Ok(names) = DiamondNameListMax200::from_readable(&name) else {
        return api_error("diamond name format or count error");
    };

    let staptr = read_mint_state(ctx);
    let state = MintStateRead::wrap(staptr.as_ref().as_ref());
    let mut cost = Amount::new();
    for dia in names.list() {
        let Some(diaobj) = state.diamond(dia) else {
            return api_error(&format!("cannot find diamond {}", dia));
        };
        if diaobj.inscripts.length() < 10 {
            continue;
        }
        let Some(diasmelt) = state.diamond_smelt(dia) else {
            return api_error(&format!("cannot find diamond {}", dia));
        };
        let camt = Amount::coin(*diasmelt.average_bid_burn as u64, 247);
        let Ok(newcost) = cost.add_mode_u128(&camt) else {
            return api_error(&format!("cannot add cost {} and {}", camt, cost));
        };
        cost = newcost;
    }

    let mut data = serde_json::Map::new();
    data.insert("cost".to_owned(), json!(cost.to_unit_string(&unit)));
    api_data(data)
}
