use std::sync::Arc;

use basis::component::*;
use basis::interface::*;
use field::*;
use protocol::context::*;
use protocol::state::*;
use protocol::transaction::*;
use serde_json::json;
use sys::*;

use crate::machine;
use crate::rt::*;
use crate::ContractAddress;
use crate::VmLog;

struct VmApiService {}

pub fn service() -> Arc<dyn ApiService> {
    Arc::new(VmApiService {})
}

impl ApiService for VmApiService {
    fn name(&self) -> &'static str {
        "vm"
    }

    fn routes(&self) -> Vec<ApiRoute> {
        vec![
            ApiRoute::get("/query/contract/sandboxcall", contract_sandbox_call),
            ApiRoute::get("/query/contract/logs", vm_logs_read),
            ApiRoute::get("/operate/contract/logs/delete", vm_logs_del),
        ]
    }
}

fn api_error(errmsg: &str) -> ApiResponse {
    ApiResponse::json(json!({"ret":1,"err":errmsg}).to_string())
}

fn api_data_raw(s: String) -> ApiResponse {
    ApiResponse::json(format!(r#"{{"ret":0,{}}}"#, s))
}

fn req_hex(s: &str) -> Ret<Vec<u8>> {
    hex::decode(s).map_err(|_| "hex format error".to_owned())
}

fn req_addr(s: &str) -> Ret<Address> {
    Address::from_readable(s).map_err(|e| format!("address {} format error: {}", s, e))
}

fn vm_logs_read(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let ck_hei = req.query_u64("height", 0);
    let index = req.query_usize("index", 0);
    let rc_hei = ctx
        .engine
        .latest_block()
        .height()
        .uint()
        .saturating_sub(ctx.engine.config().unstable_block);
    if ck_hei > rc_hei {
        return api_data_raw(s!(r#""unstable":true"#));
    }
    let logs = ctx.engine.logs();
    let Some(itdts) = logs.load(ck_hei, index) else {
        return api_data_raw(s!(r#""end":true"#));
    };
    let Ok(item) = VmLog::build(&itdts).map_ire(ItrErrCode::LogError) else {
        return api_error("log format error");
    };
    let ignore = api_data_raw(s!(r#""ignore":true"#));
    if let Some(qadr) = req.query("address") {
        let Ok(addr) = req_addr(qadr) else {
            return api_error("address format error");
        };
        if addr != item.addr {
            return ignore;
        }
    }

    macro_rules! filter_topic {
        ($key:expr, $topic:expr) => {
            if let Some(tp) = req.query($key) {
                let Ok(raw) = req_hex(tp) else {
                    return api_error("hex format error");
                };
                if raw.as_slice() != $topic.raw() {
                    return ignore;
                }
            }
        };
    }

    filter_topic!("topic0", &item.topic0);
    filter_topic!("topic1", &item.topic1);
    filter_topic!("topic2", &item.topic2);
    filter_topic!("topic3", &item.topic3);

    api_data_raw(item.render(""))
}

fn vm_logs_del(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let hei = req.query_u64("height", 0);
    ctx.engine.logs().remove(hei);
    api_data_raw(r#""ok":true"#.to_owned())
}

fn contract_sandbox_call(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let height = ctx.engine.latest_block().height().uint() + 1; // next height
    let engcnf = ctx.engine.config();
    let staptr = ctx.engine.state();
    let substa = staptr.fork_sub(Arc::downgrade(&staptr));
    let tx = TransactionType3::default();

    let env = Env {
        chain: ChainInfo {
            id: engcnf.chain_id,
            diamond_form: false,
            fast_sync: false,
        },
        block: BlkInfo {
            height,
            hash: Hash::default(),
            coinbase: Address::default(),
        },
        tx: protocol::transaction::create_tx_info(&tx),
    };
    let mut ctxobj = ContextInst::new(env, substa, Box::new(EmptyLogs {}), &tx);

    let contract = req.query("contract").unwrap_or("");
    let function = req.query("function").unwrap_or("").to_owned();
    let params = req.query("params").unwrap_or("");
    let Ok(addr) = Address::from_readable(contract) else {
        return api_error("contract address format error");
    };
    let Ok(ctrladdr) = ContractAddress::from_addr(addr) else {
        return api_error("contract address version error");
    };

    let callres = machine::sandbox_call(&mut ctxobj, ctrladdr, function, params);
    let Ok((gasuse, retval)) = callres else {
        return api_error("contract call error");
    };
    api_data_raw(format!(r#""gasuse":{},"return":{}"#, gasuse, retval))
}
