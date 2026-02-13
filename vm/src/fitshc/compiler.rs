use sys::*;

use super::parse_deploy::DeployInfo;
use super::parse_top::parse_top_level;
use super::state::ParseState;
use crate::contract::Contract;
use crate::lang::Tokenizer;
use crate::rt::SourceMap;

pub fn compile(
    code: &str,
) -> Ret<(
    Contract,
    Option<DeployInfo>,
    Vec<(String, SourceMap)>,
    String,
)> {
    let tkr = Tokenizer::new(code.as_bytes());
    let tokens = tkr.parse().map_err(|e| e.to_string())?;
    let mut state = ParseState::new(tokens);

    parse_top_level(&mut state)?;

    Ok((
        state.contract,
        state.deploy,
        state.source_maps,
        state.contract_name,
    ))
}
