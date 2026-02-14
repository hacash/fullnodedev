#[derive(Clone, Debug)]
struct ScanCoinKind {
    hacash: bool,
    satoshi: bool,
    diamond: bool,
    assets: Vec<u64>,
    assets_all: bool,
}

fn parse_scan_coinkind(raw: &str) -> Ret<ScanCoinKind> {
    let lower = raw.to_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() || compact == "all" || compact == "hsda" {
        return Ok(ScanCoinKind {
            hacash: true,
            satoshi: true,
            diamond: true,
            assets: vec![],
            assets_all: true,
        });
    }

    let mut kind_part = compact.as_str();
    let mut assets_part = "";
    if let Some(start) = compact.find('(') {
        let Some(end) = compact.rfind(')') else {
            return errf!("coinkind assets list format error");
        };
        if end <= start {
            return errf!("coinkind assets list format error");
        }
        kind_part = compact[..start].trim_matches(|c: char| c == ',' || c == '|' || c == ';');
        assets_part = &compact[start + 1..end];
    }

    let mut s = kind_part.to_owned();
    s.retain(|c| !c.is_whitespace() && c != ',' && c != ';' && c != '|');
    if s.is_empty() {
        return errf!("coinkind format error");
    }
    if !s
        .chars()
        .all(|c| c == 'h' || c == 's' || c == 'd' || c == 'a')
    {
        return errf!("coinkind format error");
    }

    let mut ck = ScanCoinKind {
        hacash: s.contains('h'),
        satoshi: s.contains('s'),
        diamond: s.contains('d'),
        assets: vec![],
        assets_all: s.contains('a'),
    };

    if !assets_part.trim().is_empty() {
        if !ck.assets_all {
            return errf!("coinkind assets list requires 'a'");
        }
        let mut list = vec![];
        for part in assets_part.split(|c| c == ',' || c == '|') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let v = p
                .parse::<u64>()
                .map_err(|_| format!("asset serial {} format error", p))?;
            let _ = Fold64::from(v)?;
            list.push(v);
        }
        if list.is_empty() {
            return errf!("coinkind assets list empty");
        }
        ck.assets = list;
        ck.assets_all = false;
    }
    Ok(ck)
}

fn scan_coin_transfer(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let coinkind_raw = q_string(&req, "coinkind", "hsda");
    let Ok(coinkind) = parse_scan_coinkind(&coinkind_raw) else {
        return api_error("coinkind format error");
    };
    let height = req.query_u64("height", 1);
    let txposi = req
        .query("txposi")
        .and_then(|v| v.parse::<isize>().ok())
        .unwrap_or(-1);
    if txposi < 0 {
        return api_error("txposi error");
    }

    let Ok(blkpkg) = load_block_by_key(ctx, &height.to_string()) else {
        return api_error("block not find");
    };
    let blkobj = &blkpkg.objc;
    let trs = blkobj.transactions();
    if trs.is_empty() {
        return api_error("transaction len error");
    }
    let txposi = txposi as usize;
    let trs = &trs[1..];
    if txposi >= trs.len() {
        return api_error("txposi overflow");
    }

    let tartrs = trs[txposi].as_read();
    let mut dtlist = vec![];
    let from_filter = req.query("from").or(req.query("filter_from"));
    let to_filter = req.query("to").or(req.query("filter_to"));
    for act in tartrs.actions() {
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

    api_data(serde_json::Map::from_iter([
        ("tx_hash".to_owned(), json!(tartrs.hash().to_hex())),
        ("tx_timestamp".to_owned(), json!(tartrs.timestamp().uint())),
        ("block_hash".to_owned(), json!(blkobj.hash().to_hex())),
        ("block_timestamp".to_owned(), json!(blkobj.timestamp().uint())),
        ("main_address".to_owned(), json!(tartrs.main().to_readable())),
        ("transfers".to_owned(), json!(dtlist)),
    ]))
}

fn append_transfer_scan(
    tx: &dyn TransactionRead,
    act: &Box<dyn Action>,
    transfers: &mut Vec<Value>,
    unit: &str,
    ck: &ScanCoinKind,
    from_filter: Option<&str>,
    to_filter: Option<&str>,
) {
    let trace = match act.kind() {
        HacToTrs::KIND | HacFromTrs::KIND | HacFromToTrs::KIND => ck.hacash,
        DiaSingleTrs::KIND | DiaFromToTrs::KIND | DiaToTrs::KIND | DiaFromTrs::KIND => ck.diamond,
        SatToTrs::KIND | SatFromTrs::KIND | SatFromToTrs::KIND => ck.satoshi,
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
    if !trace {
        return;
    }

    let mut obj = action_to_json_desc(tx, act, unit, false);
    let adrs = tx.addrs();
    let main_addr = tx.main().to_readable();
    let must_addr = |a: AddrOrPtr| -> String { a.real(&adrs).unwrap().to_readable() };

    if let Some(a) = HacToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(main_addr.clone()));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
    } else if let Some(a) = HacFromTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(main_addr.clone()));
    } else if let Some(a) = HacFromToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
    } else if let Some(a) = SatToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(main_addr.clone()));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
    } else if let Some(a) = SatFromTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(main_addr.clone()));
    } else if let Some(a) = SatFromToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
    } else if let Some(a) = DiaSingleTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(main_addr.clone()));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
        obj.insert("diamond".to_owned(), json!(1u32));
        obj.insert("diamonds".to_owned(), json!(a.diamond.to_readable()));
    } else if let Some(a) = DiaToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(main_addr.clone()));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
        obj.insert("diamond".to_owned(), json!(a.diamonds.length()));
        obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
    } else if let Some(a) = DiaFromTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(main_addr.clone()));
        obj.insert("diamond".to_owned(), json!(a.diamonds.length()));
        obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
    } else if let Some(a) = DiaFromToTrs::downcast(act) {
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
        obj.insert("diamond".to_owned(), json!(a.diamonds.length()));
        obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
    } else if let Some(a) = AssetToTrs::downcast(act) {
        if !(ck.assets_all || ck.assets.contains(&a.asset.serial.uint())) {
            return;
        }
        obj.insert("from".to_owned(), json!(main_addr.clone()));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
    } else if let Some(a) = AssetFromTrs::downcast(act) {
        if !(ck.assets_all || ck.assets.contains(&a.asset.serial.uint())) {
            return;
        }
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(main_addr.clone()));
    } else if let Some(a) = AssetFromToTrs::downcast(act) {
        if !(ck.assets_all || ck.assets.contains(&a.asset.serial.uint())) {
            return;
        }
        obj.insert("from".to_owned(), json!(must_addr(a.from)));
        obj.insert("to".to_owned(), json!(must_addr(a.to)));
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
    transfers.push(json!(obj));
}
