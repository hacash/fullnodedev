
use serde_json::Value;

// action from json
pub fn action_from_json(json_str: &str) -> Ret<Box<dyn Action>> {
    let jsonv: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("action json parse error: {}", e))?;

    let Some(..) = jsonv.as_object() else {
        return errf!("action format error")
    };

    let Some(kind) = jsonv["kind"].as_u64() else {
        return errf!("kind format error")
    };
    if kind > u16::MAX as u64 {
        return errf!("kind {} value overflow", kind)
    }
    
    // decode
    protocol::action::action_json_create(kind as u16, json_str)?
        .ok_or_else(|| format!("kind {} not found in registry", kind))
}



// action to json desc
pub fn action_to_json_desc(_tx: &dyn TransactionRead, act: &Box<dyn Action>, 
    unit: &str, ret_desc: bool
) -> JsonObject {
    let json_str = act.to_json_fmt(&JSONFormater::new_unit(unit));
    let jsonv: Value = serde_json::from_str(&json_str).unwrap_or(serde_json::json!({}));
    
    let mut resjsonobj = JsonObject::new();
    if let Value::Object(map) = jsonv {
        for (k, v) in map {
            resjsonobj.insert(Box::leak(k.into_boxed_str()), v);
        }
    }

    // add kind
    resjsonobj.insert("kind", serde_json::json!(act.kind()));

    if ret_desc {
        let desc = act.to_description();
        resjsonobj.insert("description", serde_json::json!(desc));
    }
    resjsonobj
}
