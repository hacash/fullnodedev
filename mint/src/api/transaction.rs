fn transaction_sign(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let prikey = q_string(&req, "prikey", "");
    let pubkey = q_string(&req, "pubkey", "");
    let sigdts = q_string(&req, "sigdts", "");
    let signature = q_bool(&req, "signature", false);
    let description = q_bool(&req, "description", false);

    let lasthei = ctx.engine.latest_block().height().uint();
    let Ok(txdts) = body_data_may_hex(&req) else {
        return api_error("transaction body error");
    };
    let Ok((mut tx, _)) = protocol::transaction::transaction_create(&txdts) else {
        return api_error("transaction body error");
    };

    let (address, signobj) = if prikey.len() == 64 {
        let Ok(prik) = hex::decode(&prikey) else {
            return api_error("prikey format error");
        };
        let Ok(acc) = Account::create_by_secret_key_value(prik.try_into().unwrap()) else {
            return api_error("prikey data error");
        };
        let fres = tx.fill_sign(&acc);
        if let Err(e) = fres {
            return api_error(&format!("fill sign error: {}", e));
        }
        (Address::from(*acc.address()), fres.unwrap())
    } else {
        if pubkey.len() != 33 * 2 || sigdts.len() != 64 * 2 {
            return api_error("pubkey or signature data error");
        }
        let Ok(pbk) = hex::decode(&pubkey) else {
            return api_error("pubkey format error");
        };
        let Ok(sig) = hex::decode(&sigdts) else {
            return api_error("sigdts format error");
        };
        let pbk: [u8; 33] = pbk.try_into().unwrap();
        let sig: [u8; 64] = sig.try_into().unwrap();
        let signobj = field::Sign {
            publickey: Fixed33::from(pbk),
            signature: Fixed64::from(sig),
        };
        if let Err(e) = tx.push_sign(signobj.clone()) {
            return api_error(&format!("fill sign error: {}", e));
        }
        (Address::from(Account::get_address_by_public_key(pbk)), signobj)
    };

    let mut data = render_tx_info(
        tx.as_read(),
        None,
        lasthei,
        &unit,
        true,
        signature,
        false,
        description,
    );
    data.insert(
        "sign_data".to_owned(),
        json!({
            "address": address.to_readable(),
            "pubkey": signobj.publickey.to_hex(),
            "sigdts": signobj.signature.to_hex(),
        }),
    );
    api_data(data)
}

fn transaction_build(_ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let action = q_bool(&req, "action", false);
    let signature = q_bool(&req, "signature", false);
    let description = q_bool(&req, "description", false);

    let Ok(txjsondts) = body_data_may_hex(&req) else {
        return api_error("transaction json body error");
    };
    let Ok(jsonstr) = std::str::from_utf8(&txjsondts) else {
        return api_error("transaction json body error");
    };
    let Ok(jsonv) = serde_json::from_str::<serde_json::Value>(jsonstr) else {
        return api_error("transaction json body error");
    };

    let Some(main_addr) = jsonv["main_address"].as_str() else {
        return api_error("address format error");
    };
    let Ok(main_addr) = Address::from_readable(main_addr) else {
        return api_error("address format error");
    };
    let Some(fee) = jsonv["fee"].as_str() else {
        return api_error("amount format error");
    };
    let Ok(fee) = Amount::from(fee) else {
        return api_error("amount format error");
    };

    let mut tx = TransactionType2::new_by(main_addr, fee, curtimes());
    if let Some(ts) = jsonv["timestamp"].as_u64() {
        tx.timestamp = Timestamp::from(ts);
    }

    let Some(acts) = jsonv["actions"].as_array() else {
        return api_error("actions format error");
    };
    for act in acts {
        let Ok(a) = action_from_json(&act.to_string()) else {
            return api_error("push action error");
        };
        if tx.push_action(a).is_err() {
            return api_error("push action error");
        }
    }

    api_data(render_tx_info(
        tx.as_read(),
        None,
        0,
        &unit,
        true,
        signature,
        action,
        description,
    ))
}

fn transaction_check(_ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let set_fee = q_string(&req, "set_fee", "");
    let sign_address = q_string(&req, "sign_address", "");
    let body = q_bool(&req, "body", false);
    let signature = q_bool(&req, "signature", false);
    let description = q_bool(&req, "description", false);

    let Ok(txdts) = body_data_may_hex(&req) else {
        return api_error("transaction body error");
    };
    let Ok((mut tx, _)) = protocol::transaction::transaction_create(&txdts) else {
        return api_error("transaction body error");
    };

    if !set_fee.is_empty() {
        let Ok(fee) = Amount::from(&set_fee) else {
            return api_error("fee format error");
        };
        tx.set_fee(fee);
    }

    let tx = tx.as_read();
    let mut data = render_tx_info(tx, None, 0, &unit, body, signature, true, description);
    if !sign_address.is_empty() {
        let Ok(addr) = Address::from_readable(&sign_address) else {
            return api_error("sign_address format error");
        };
        let sign_hash = if tx.main() == addr {
            tx.hash_with_fee()
        } else {
            tx.hash()
        };
        data.insert("sign_hash".to_owned(), json!(sign_hash.to_hex()));
    }

    api_data(data)
}

fn transaction_exist(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let hash = q_string(&req, "hash", "");
    let body = q_bool(&req, "body", false);
    let action = q_bool(&req, "action", false);
    let signature = q_bool(&req, "signature", false);
    let description = q_bool(&req, "description", false);
    let lasthei = ctx.engine.latest_block().height().uint();

    let Ok(hx) = hex::decode(&hash) else {
        return api_error("transaction hash format error");
    };
    if hx.len() != Hash::SIZE {
        return api_error("transaction hash format error");
    }
    let txhx = Hash::must(&hx);

    let txpool = ctx.hnoder.txpool();
    if let Some(txp) = txpool.find(&txhx) {
        let mut info = render_tx_info(
            txp.objc.as_read(),
            None,
            lasthei,
            &unit,
            body,
            signature,
            action,
            description,
        );
        info.insert("pending".to_owned(), json!(true));
        return api_data(info);
    }

    let state_ptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(state_ptr.as_ref().as_ref());
    let Some(txp) = state.tx_exist(&txhx) else {
        return api_error("transaction not find");
    };
    let Ok(blkpkg) = load_block_by_key(ctx, &txp.to_string()) else {
        return api_error("cannot find block by transaction ptr");
    };
    let blkobj = &blkpkg.objc;
    let blktrs = blkobj.transactions();

    let tx = {
        let txnum = blkobj.transaction_count().uint() as usize;
        let mut found = None;
        for it in blktrs[1..txnum].iter() {
            if txhx == it.hash() {
                found = Some(it.clone());
                break;
            }
        }
        found
    };
    let Some(tx) = tx else {
        return api_error("transaction not find in the block");
    };

    api_data(render_tx_info(
        tx.as_read(),
        Some(blkobj.as_read()),
        lasthei,
        &unit,
        body,
        signature,
        action,
        description,
    ))
}

fn render_tx_info(
    tx: &dyn TransactionRead,
    blblk: Option<&dyn BlockRead>,
    lasthei: u64,
    unit: &str,
    body: bool,
    signature: bool,
    action: bool,
    description: bool,
) -> serde_json::Map<String, Value> {
    let fee_str = tx.fee().to_unit_string(unit);
    let main_addr = tx.main().to_readable();
    let mut data = serde_json::Map::new();
    data.insert("hash".to_owned(), json!(tx.hash().to_hex()));
    data.insert("hash_with_fee".to_owned(), json!(tx.hash_with_fee().to_hex()));
    data.insert("type".to_owned(), json!(tx.ty()));
    data.insert("timestamp".to_owned(), json!(tx.timestamp().uint()));
    data.insert("fee".to_owned(), json!(fee_str));
    data.insert("fee_got".to_owned(), json!(tx.fee_got().to_unit_string(unit)));
    data.insert("main_address".to_owned(), json!(main_addr.clone()));
    data.insert("action".to_owned(), json!(tx.action_count()));

    if body {
        data.insert("body".to_owned(), json!(tx.serialize().to_hex()));
    }
    if signature {
        check_signature(&mut data, tx);
    }
    if description {
        data.insert(
            "description".to_owned(),
            json!(format!("Main account {} pay {} HAC tx fee", main_addr, fee_str)),
        );
    }
    if let Some(blkobj) = blblk {
        let txblkhei = blkobj.height().uint();
        data.insert(
            "block".to_owned(),
            json!({
                "height": txblkhei,
                "timestamp": blkobj.timestamp().uint(),
            }),
        );
        data.insert("confirm".to_owned(), json!(lasthei - txblkhei));
    }
    if action {
        let mut acts = Vec::with_capacity(tx.actions().len());
        for act in tx.actions() {
            acts.push(action_to_json_desc(tx, act, unit, description));
        }
        data.insert("actions".to_owned(), json!(acts));
    }
    data
}

fn check_signature(data: &mut serde_json::Map<String, Value>, tx: &dyn TransactionRead) {
    let Ok(sigstats) = check_tx_signature(tx) else {
        return;
    };
    let mut sigchs = vec![];
    for (adr, sg) in sigstats {
        sigchs.push(json!({
            "address": adr.to_readable(),
            "complete": sg,
        }));
    }
    data.insert("signatures".to_owned(), json!(sigchs));
}
