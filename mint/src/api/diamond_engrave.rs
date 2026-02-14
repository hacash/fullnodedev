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
