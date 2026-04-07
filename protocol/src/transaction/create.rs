pub fn transaction_create(buf: &[u8]) -> Ret<(Box<dyn Transaction>, usize)> {
    let ty = bufeatone(buf)?;
    let registry = crate::setup::current_setup()?;
    let Some(codec) = registry.tx_codecs.get(&ty).copied() else {
        return errf!("transaction type '{}' not found", ty);
    };
    (codec.create)(buf)
}

pub fn try_json_decode(ty: u8, json: &str) -> Ret<Option<Box<dyn Transaction>>> {
    let registry = crate::setup::current_setup()?;
    let Some(codec) = registry.tx_codecs.get(&ty).copied() else {
        return Ok(None);
    };
    (codec.json_decode)(json).map(Some)
}

pub fn transaction_json_decode(json: &str) -> Ret<Option<Box<dyn Transaction>>> {
    let obj = json_decode_object(json)?;
    let ty_str = obj
        .get("ty")
        .ok_or_else(|| "transaction object JSON must have 'ty'".to_string())?;
    let ty = ty_str
        .parse::<u8>()
        .map_err(|_| format!("invalid transaction type: {}", ty_str))?;
    try_json_decode(ty, json)
}
