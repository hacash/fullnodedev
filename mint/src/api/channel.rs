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
