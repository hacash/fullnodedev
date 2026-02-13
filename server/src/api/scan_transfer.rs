
api_querys_define!{ Q4538,
    height, u64, 1,
    txposi, isize, -1,
    r#from, Option<String>, None,
    r#to, Option<String>, None,
    filter_from, Option<String>, None,
    filter_to, Option<String>, None,
}

async fn scan_coin_transfer(State(ctx): State<ApiCtx>, q: Query<Q4538>) -> impl IntoResponse {
    ctx_store!(ctx, store);
    q_unit!(q, unit);
    q_coinkind!(q, coinkind);
    let blkpkg = api_load_block(&ctx, store.as_ref(), &q.height.to_string());
    if let Err(e) = blkpkg {
        return  api_error(&e)
    }
    let blkobj = blkpkg.unwrap();
    let blkobj = &blkobj.objc;
    let trs = blkobj.transactions();
    if trs.len() == 0 {
        return api_error("transaction len error")
    }
    if q.txposi < 0 {
        return api_error("txposi error")
    }
    let txposi = q.txposi as usize;
    let trs = &trs[1..];
    if txposi >= trs.len() {
        return api_error("txposi overflow")
    }
    let tartrs = trs[txposi].as_read();
    let mainaddr_readable = tartrs.main().to_readable();
    let mut dtlist = Vec::new();
    let from_filter = q.r#from.as_deref().or(q.filter_from.as_deref());
    let to_filter = q.r#to.as_deref().or(q.filter_to.as_deref());
    // scan actions
    for act in tartrs.actions()  {
        append_transfer_scan(
            tartrs,
            act,
            &mut dtlist,
            &unit,
            &coinkind,
            from_filter,
            to_filter,
        );
    }
    // ok
    let data = jsondata!{
        "tx_hash", tartrs.hash().to_hex(),
        "tx_timestamp", tartrs.timestamp().uint(),
        "block_hash", blkobj.hash().to_hex(),
        "block_timestamp", blkobj.timestamp().uint(),
        "main_address", mainaddr_readable,
        "transfers", dtlist,
    };
    api_data(data)
}



fn append_transfer_scan(tx: &dyn TransactionRead, act: &Box<dyn Action>, 
    transfers: &mut Vec<JsonObject>, unit: &String, ck: &CoinKind,
    from_filter: Option<&str>, to_filter: Option<&str>,
) {
    let trace = match act.kind() {

        HacToTrs::KIND |
        HacFromTrs::KIND |
        HacFromToTrs::KIND => ck.hacash,

        DiaSingleTrs::KIND |
        DiaFromToTrs::KIND |
        DiaToTrs::KIND |
        DiaFromTrs::KIND => ck.diamond,

        SatToTrs::KIND |
        SatFromTrs::KIND |
        SatFromToTrs::KIND => ck.satoshi,

        _ => false,
    };
    
    let mut trace = trace;
    if !trace && (ck.assets_all || !ck.assets.is_empty()) {
        if let Some(a) = AssetToTrs::downcast(act) {
            trace = ck.assets_all || ck.assets.contains(&a.asset.serial.uint());
        } else if let Some(a) = AssetFromTrs::downcast(act) {
            trace = ck.assets_all || ck.assets.contains(&a.asset.serial.uint());
        } else if let Some(a) = AssetFromToTrs::downcast(act) {
            trace = ck.assets_all || ck.assets.contains(&a.asset.serial.uint());
        }
    }
    
    if ! trace { return }

    // append
    let mut obj = action_to_json_desc(tx, act, unit, false);
    let adrs = tx.addrs();
    let main_addr = tx.main().to_readable();
    macro_rules! must_addr {
        ( $k:expr ) => {{
            $k.real(&adrs).unwrap().to_readable()
        }}
    }
    macro_rules! set_from_to {
        ( $from:expr, $to:expr ) => {{
            obj.insert("from", json!($from));
            obj.insert("to", json!($to));
        }}
    }
    macro_rules! set_from_to_ptr {
        ( $from:expr, $to:expr ) => {{
            let from_addr = must_addr!($from);
            let to_addr = must_addr!($to);
            set_from_to!(from_addr, to_addr);
        }}
    }
    macro_rules! set_diamond_list {
        ( $list:expr ) => {{
            obj.insert("diamond", json!($list.length()));
            obj.insert("diamonds", json!($list.readable()));
        }}
    }
    macro_rules! check_asset {
        ( $a:expr ) => {{
            if !(ck.assets_all || ck.assets.contains(&$a.asset.serial.uint())) { return }

        }}
    }
    if let Some(a) = HacToTrs::downcast(act) {
        let to_addr = must_addr!(a.to);
        set_from_to!(main_addr.clone(), to_addr);
    } else if let Some(a) = HacFromTrs::downcast(act) {
        let from_addr = must_addr!(a.from);
        set_from_to!(from_addr, main_addr.clone());
    } else if let Some(a) = HacFromToTrs::downcast(act) {
        set_from_to_ptr!(a.from, a.to);
    } else if let Some(a) = SatToTrs::downcast(act) {
        let to_addr = must_addr!(a.to);
        set_from_to!(main_addr.clone(), to_addr);
    } else if let Some(a) = SatFromTrs::downcast(act) {
        let from_addr = must_addr!(a.from);
        set_from_to!(from_addr, main_addr.clone());
    } else if let Some(a) = SatFromToTrs::downcast(act) {
        set_from_to_ptr!(a.from, a.to);
    } else if let Some(a) = DiaSingleTrs::downcast(act) {
        let to_addr = must_addr!(a.to);
        set_from_to!(main_addr.clone(), to_addr);
        obj.insert("diamond", json!(1u32));
        obj.insert("diamonds", json!(a.diamond.to_readable()));
    } else if let Some(a) = DiaToTrs::downcast(act) {
        let to_addr = must_addr!(a.to);
        set_from_to!(main_addr.clone(), to_addr);
        set_diamond_list!(a.diamonds);
    } else if let Some(a) = DiaFromTrs::downcast(act) {
        let from_addr = must_addr!(a.from);
        set_from_to!(from_addr, main_addr.clone());
        set_diamond_list!(a.diamonds);
    } else if let Some(a) = DiaFromToTrs::downcast(act) {
        set_from_to_ptr!(a.from, a.to);
        set_diamond_list!(a.diamonds);
    } else if let Some(a) = AssetToTrs::downcast(act) {
        check_asset!(a);
        let to_addr = must_addr!(a.to);
        set_from_to!(main_addr.clone(), to_addr);
    } else if let Some(a) = AssetFromTrs::downcast(act) {
        check_asset!(a);
        let from_addr = must_addr!(a.from);
        set_from_to!(from_addr, main_addr.clone());
    } else if let Some(a) = AssetFromToTrs::downcast(act) {
        check_asset!(a);
        set_from_to_ptr!(a.from, a.to);
    }
    if let Some(filter_from) = from_filter {
        let from_addr = obj.get("from").and_then(|v| v.as_str()).unwrap_or("");
        if from_addr != filter_from {
            return;
        }
    }
    if let Some(filter_to) = to_filter {
        let to_addr = obj.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if to_addr != filter_to {
            return;
        }
    }
    transfers.push(obj);
    
}
