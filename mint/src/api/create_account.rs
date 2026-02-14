fn account(_ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let quantity = req.query_u64("quantity", 1);
    if quantity == 0 {
        return api_error("quantity error");
    }
    if quantity > 200 {
        return api_error("quantity max 200");
    }

    let mut resbls = Vec::with_capacity(quantity as usize);
    for _ in 0..quantity {
        let acc = Account::create_randomly(&|data| {
            getrandom::fill(data).map_err(|e| e.to_string())?;
            Ok(())
        });
        let Ok(acc) = acc else {
            return api_error("create account error");
        };
        resbls.push(json!({
            "address": acc.readable(),
            "prikey": hex::encode(acc.secret_key().serialize()),
            "pubkey": hex::encode(acc.public_key().serialize_compressed()),
        }));
    }

    api_data_list(resbls)
}
