fn create_coin_transfer(_ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let fee = q_string(&req, "fee", "");
    let main_prikey = q_string(&req, "main_prikey", "");
    let to_address = q_string(&req, "to_address", "");
    let timestamp = req.query_u64("timestamp", 0);
    let from_prikey = q_string(&req, "from_prikey", "");
    let hacash = q_string(&req, "hacash", "");
    let satoshi = req.query_u64("satoshi", 0);
    let diamonds = q_string(&req, "diamonds", "");

    let Ok(toaddr) = Address::from_readable(&to_address) else {
        return api_error("to_address format error");
    };
    let Ok(fee) = Amount::from(&fee) else {
        return api_error("fee format error");
    };
    let Ok(main_acc) = Account::create_by(&main_prikey) else {
        return api_error("main_prikey format error");
    };

    let mut from_acc = main_acc.clone();
    if !from_prikey.is_empty() {
        let Ok(acc) = Account::create_by(&from_prikey) else {
            return api_error("from_prikey format error");
        };
        from_acc = acc;
    }
    let is_from = from_acc != main_acc;
    let addr = Address::from(main_acc.address().clone());
    let fromaddr = Address::from(from_acc.address().clone());

    let mut tx = TransactionType2::new_by(addr, fee, curtimes());
    if timestamp > 0 {
        tx.timestamp = Timestamp::from(timestamp);
    }

    if satoshi > 0 {
        let sat = Satoshi::from(satoshi);
        let act: Box<dyn Action> = if is_from {
            let mut obj = SatFromToTrs::new();
            obj.from = AddrOrPtr::from_addr(fromaddr.clone());
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.satoshi = sat;
            Box::new(obj)
        } else {
            let mut obj = SatToTrs::new();
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.satoshi = sat;
            Box::new(obj)
        };
        if tx.push_action(act).is_err() {
            return api_error("push sat action error");
        }
    }

    if diamonds.len() >= DiamondName::SIZE {
        let Ok(dialist) = DiamondNameListMax200::from_readable(&diamonds) else {
            return api_error("diamonds format error");
        };
        let act: Box<dyn Action> = if is_from {
            let mut obj = DiaFromToTrs::new();
            obj.from = AddrOrPtr::from_addr(fromaddr.clone());
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.diamonds = dialist;
            Box::new(obj)
        } else if dialist.length() == 1 {
            let mut obj = DiaSingleTrs::new();
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.diamond = DiamondName::from(*dialist.as_list()[0]);
            Box::new(obj)
        } else {
            let mut obj = DiaToTrs::new();
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.diamonds = dialist;
            Box::new(obj)
        };
        if tx.push_action(act).is_err() {
            return api_error("push diamond action error");
        }
    }

    if !hacash.is_empty() {
        let Ok(hac) = Amount::from(&hacash) else {
            return api_error("hacash amount format error");
        };
        let act: Box<dyn Action> = if is_from {
            let mut obj = HacFromToTrs::new();
            obj.from = AddrOrPtr::from_addr(fromaddr.clone());
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.hacash = hac;
            Box::new(obj)
        } else {
            let mut obj = HacToTrs::new();
            obj.to = AddrOrPtr::from_addr(toaddr.clone());
            obj.hacash = hac;
            Box::new(obj)
        };
        if tx.push_action(act).is_err() {
            return api_error("push hac action error");
        }
    }

    if let Err(e) = tx.fill_sign(&main_acc) {
        return api_error(&format!("fill main sign error: {}", e));
    }
    if is_from {
        if let Err(e) = tx.fill_sign(&from_acc) {
            return api_error(&format!("fill from sign error: {}", e));
        }
    }

    api_data(serde_json::Map::from_iter([
        ("hash".to_owned(), json!(tx.hash().to_hex())),
        ("hash_with_fee".to_owned(), json!(tx.hash_with_fee().to_hex())),
        ("timestamp".to_owned(), json!(tx.timestamp().uint())),
        ("body".to_owned(), json!(tx.serialize().to_hex())),
    ]))
}
