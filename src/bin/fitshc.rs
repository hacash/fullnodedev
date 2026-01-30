use std::env;
use std::fs;
use std::path::Path;
use vm::action::ContractDeploy;
use field::*;
use vm::fitshc::compiler::compile;
// use sys::*;
use serde_json::{json, Value};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: fitshc <file.fitsh> [fee] [nonce]");
        return;
    }
    let file_path = &args[1];
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
             println!("Error reading file: {}", e);
             return;
        }
    };

    let (contract, deploy_opt, smaps, contract_name) = match compile(&source) {
        Ok(res) => res,
        Err(e) => {
            println!("Compile error: {:?}", e);
            return;
        }
    };

    println!("Compile success!");

    let path = Path::new(file_path);
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or("output".into());
    let parent = path.parent().unwrap_or(Path::new("."));

    let sto = contract.into_sto();

    // Map
    let mut func_maps = Vec::new();
    for (name, sm) in smaps {
        if let Ok(json_str) = sm.to_json() {
             if let Ok(mut v) = serde_json::from_str::<Value>(&json_str) {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("name".to_string(), json!(name));
                }
                func_maps.push(v);
            }
        }
    }

    // Access using trait or method if available, but ContractAddrsssW1 seems to not expose lists.
    // However, it implements combi_list, which provides `list()` method usually or `lists` field.
    // `combi_list` macro defined in basis/macros? or field/macros?
    // Let's assume list() method exists based on earlier usage in contract.rs: `self.inherits.length()`.
    // Wait, earlier code `self.abstcalls.list()` works.
    let inherit_addrs: Vec<String> = sto.inherits.list().iter().map(|a| a.readable()).collect();
    let lib_addrs: Vec<String> = sto.librarys.list().iter().map(|a| a.readable()).collect();
    
    let contract_map = json!({
        "contract": contract_name,
        "inherits": inherit_addrs,
        "libs": lib_addrs,
        "funcs": func_maps
    });

    let map_file = parent.join(format!("{}.contractmap.json", stem));
    fs::write(&map_file, serde_json::to_string_pretty(&contract_map).unwrap()).ok();
    println!("Generated: {}", map_file.display());

    // Deploy
    let (d_fee, d_nonce, d_argv) = if let Some(info) = deploy_opt {
        (info.protocol_cost, info.nonce, info.construct_argv)
    } else {
        (None, None, None)
    };
    
    let fee_str = args.get(2).map(|s| s.as_str()).unwrap_or("1:248"); 
    let fee = d_fee.unwrap_or_else(|| Amount::from(fee_str).unwrap_or(Amount::default())); 
    
    let nonce_val = args.get(3).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let nonce = d_nonce.unwrap_or(Uint4::from(nonce_val));
    
    let argv = d_argv.unwrap_or_default(); 
    
    let mut action = ContractDeploy::default();
    action.protocol_cost = fee;
    action.nonce = nonce;
    action.construct_argv = argv;
    action.contract = sto;
    
    let action_bytes = action.serialize();
    let deploy_json = json!({
        "action": hex::encode(&action_bytes)
    });

    let deploy_file = parent.join(format!("{}.deploy.json", stem));
    fs::write(&deploy_file, serde_json::to_string_pretty(&deploy_json).unwrap()).ok();
    println!("Generated: {}", deploy_file.display());
}
