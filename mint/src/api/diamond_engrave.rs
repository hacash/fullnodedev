fn push_diamond_engrave_item(
    datalist: &mut Vec<Value>,
    txhx: &Hash,
    with_tx_hash: bool,
    mut obj: serde_json::Map<String, Value>,
) {
    if with_tx_hash {
        obj.insert("tx_hash".to_owned(), json!(txhx.to_hex()));
    }
    datalist.push(Value::Object(obj));
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
            if let Some(a) = action::DiaInscPush::downcast(act) {
                let mut obj = serde_json::Map::new();
                obj.insert("action".to_owned(), json!("inscription"));
                obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
                obj.insert(
                    "inscription".to_owned(),
                    json!(a.engraved_content.to_readable_or_hex()),
                );
                obj.insert("engraved_type".to_owned(), json!(*a.engraved_type));
                obj.insert(
                    "protocol_cost".to_owned(),
                    json!(a.protocol_cost.to_fin_string()),
                );
                push_diamond_engrave_item(&mut datalist, &txhx, tx_hash, obj);
            } else if let Some(a) = action::DiaInscClean::downcast(act) {
                let mut obj = serde_json::Map::new();
                obj.insert("action".to_owned(), json!("clear"));
                obj.insert("diamonds".to_owned(), json!(a.diamonds.readable()));
                obj.insert(
                    "protocol_cost".to_owned(),
                    json!(a.protocol_cost.to_fin_string()),
                );
                push_diamond_engrave_item(&mut datalist, &txhx, tx_hash, obj);
            } else if let Some(a) = action::DiaInscMove::downcast(act) {
                let from = a.from_diamond.to_readable();
                let to = a.to_diamond.to_readable();
                let mut obj = serde_json::Map::new();
                obj.insert("action".to_owned(), json!("move"));
                // Use `diamonds` as unified literal carrier (source + target).
                obj.insert("diamonds".to_owned(), json!(format!("{}{}", from, to)));
                obj.insert(
                    "index".to_owned(),
                    json!(*a.index as u64),
                );
                obj.insert(
                    "protocol_cost".to_owned(),
                    json!(a.protocol_cost.to_fin_string()),
                );
                push_diamond_engrave_item(&mut datalist, &txhx, tx_hash, obj);
            } else if let Some(a) = action::DiaInscDrop::downcast(act) {
                let dia = a.diamond.to_readable();
                let mut obj = serde_json::Map::new();
                obj.insert("action".to_owned(), json!("drop"));
                obj.insert("diamonds".to_owned(), json!(dia));
                obj.insert(
                    "index".to_owned(),
                    json!(*a.index as u64),
                );
                obj.insert(
                    "protocol_cost".to_owned(),
                    json!(a.protocol_cost.to_fin_string()),
                );
                push_diamond_engrave_item(&mut datalist, &txhx, tx_hash, obj);
            } else if let Some(a) = action::DiaInscEdit::downcast(act) {
                let dia = a.diamond.to_readable();
                let mut obj = serde_json::Map::new();
                obj.insert("action".to_owned(), json!("edit"));
                obj.insert("diamonds".to_owned(), json!(dia));
                obj.insert(
                    "index".to_owned(),
                    json!(*a.index as u64),
                );
                obj.insert("engraved_type".to_owned(), json!(*a.engraved_type));
                obj.insert(
                    "protocol_cost".to_owned(),
                    json!(a.protocol_cost.to_fin_string()),
                );
                obj.insert(
                    "inscription".to_owned(),
                    json!(a.engraved_content.to_readable_or_hex()),
                );
                push_diamond_engrave_item(&mut datalist, &txhx, tx_hash, obj);
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
