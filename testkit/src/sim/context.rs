use basis::component::Env;
use basis::interface::{Logs, State, TransactionRead};
use protocol::context::ContextInst;
use protocol::state::EmptyLogs;
use protocol::transaction::TransactionType2;

pub fn make_ctx_with_state<'a>(
    env: Env,
    state: Box<dyn State>,
    tx: &'a dyn TransactionRead,
) -> ContextInst<'a> {
    make_ctx_with_logs(env, state, Box::new(EmptyLogs {}), tx)
}

pub fn make_ctx_with_logs<'a>(
    env: Env,
    state: Box<dyn State>,
    logs: Box<dyn Logs>,
    tx: &'a dyn TransactionRead,
) -> ContextInst<'a> {
    ContextInst::new(env, state, logs, tx)
}

pub fn make_ctx_with_default_tx(env: Env, state: Box<dyn State>) -> ContextInst<'static> {
    static DEFAULT_TX: std::sync::OnceLock<TransactionType2> = std::sync::OnceLock::new();
    let tx = DEFAULT_TX.get_or_init(TransactionType2::default);
    make_ctx_with_state(env, state, tx)
}
